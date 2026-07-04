use super::*;
use crate::ModelTurnResponse;
use agent_os_kernel::{
    AttachEvidenceInput, CommitMemoryWriteInput, CompactContextInput, ForkThreadInput,
    LoadContextInput, ProposeMemoryWriteInput, RegisterGoalInput, RollbackThreadInput,
    SpawnAgentInput, SpawnTaskInput,
};
use std::{
    collections::VecDeque,
    env, fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone)]
enum DeterministicStep {
    ToolCall(ToolAction),
    Final {
        summary: String,
        known_risks: Vec<String>,
        tests_run: Vec<String>,
        tests_not_run: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct DeterministicModelClient {
    steps: VecDeque<DeterministicStep>,
}

impl DeterministicModelClient {
    fn new(steps: impl IntoIterator<Item = DeterministicStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

impl ModelClient for DeterministicModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        let step = self.steps.pop_front().ok_or_else(|| {
            AgentOsError::Validation(
                "deterministic model exhausted before final submission".to_string(),
            )
        })?;
        let action = match step {
            DeterministicStep::ToolCall(action) => ModelAction::ToolCall(action),
            DeterministicStep::Final {
                summary,
                known_risks,
                tests_run,
                tests_not_run,
            } => {
                let mut evidence_map = Vec::new();
                for result in &request.context.tool_results {
                    if result.evidence_ids.is_empty() {
                        continue;
                    }
                    let claim = result.evidence_claim.clone().ok_or_else(|| {
                        AgentOsError::Validation(format!(
                            "tool {} omitted evidence claim",
                            result.tool_name
                        ))
                    })?;
                    evidence_map.push(EvidenceMapEntry {
                        claim,
                        evidence_refs: result.evidence_ids.clone(),
                    });
                }
                ModelAction::Final {
                    submission: FinalSubmission {
                        summary,
                        changed_artifacts: request
                            .context
                            .artifacts
                            .iter()
                            .map(|artifact| artifact.artifact_id.clone())
                            .collect(),
                        evidence_map,
                        unverified_claims: Vec::new(),
                        known_risks,
                        tests_run,
                        tests_not_run,
                        approvals: Vec::new(),
                    },
                }
            }
        };
        Ok(ModelTurnResponse {
            actions: vec![action],
            usage: ProviderUsage {
                input_tokens: request.thread.task.goal.len() as u64,
                output_tokens: 1,
                cost: 0.0,
            },
        })
    }
}

#[test]
fn runtime_job_projects_active_turn_contract() {
    let workspace = temp_workspace("runtime-job-contract");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Run job contract");
    let active = kernel.start_turn(&agent.thread_id).unwrap();

    let job = RuntimeJob::from_active_turn(&active).unwrap();

    assert_eq!(job.client_thread_id, agent.thread_id);
    assert_eq!(job.agent_thread_id, agent.thread_id);
    assert_eq!(job.turn_id, active.active_turn.turn_id.clone().unwrap());
    assert_eq!(job.workspace, workspace.to_string_lossy().to_string());
    assert_eq!(
        job.provider_profile,
        active.config_snapshot.provider_profile_id
    );
    assert_eq!(job.model, active.config_snapshot.model_id);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_job_runner_uses_existing_turn_without_starting_another() {
    let workspace = temp_workspace("runtime-job-existing-turn");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Finish existing turn");
    let active = kernel.start_turn(&agent.thread_id).unwrap();
    let job = RuntimeJob::from_active_turn(&active).unwrap();
    let current_exe = env::current_exe().unwrap();
    let script = DeterministicModelClient::new(vec![
        DeterministicStep::ToolCall(ToolAction::new(
            "run_command",
            json!({
                "mode": "exec",
                "command": current_exe.to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace.to_string_lossy()
            }),
            4,
            Some("runtime job command evidence was captured".to_string()),
        )),
        DeterministicStep::Final {
            summary: "Finished from an existing host turn.".to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["test binary --help".to_string()],
            tests_not_run: Vec::new(),
        },
    ]);

    let mut runtime = ThreadRuntime::new_for_job(kernel.clone(), job, script);
    let report = runtime
        .run_job_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(
        kernel
            .events()
            .unwrap()
            .into_iter()
            .filter(|event| event.event_type == "TurnStarted")
            .count(),
        1
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_job_runner_stops_before_model_call_when_turn_is_interrupted() {
    struct PanicIfCalled;

    impl ModelClient for PanicIfCalled {
        fn next(&mut self, _request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            panic!("interrupted runtime job called the model")
        }
    }

    let workspace = temp_workspace("runtime-job-interrupted");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Stop interrupted turn");
    let active = kernel.start_turn(&agent.thread_id).unwrap();
    let job = RuntimeJob::from_active_turn(&active).unwrap();
    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Interrupted,
            Some("test interrupt".to_string()),
        )
        .unwrap();

    let mut runtime = ThreadRuntime::new_for_job(kernel.clone(), job, PanicIfCalled);
    let report = runtime
        .run_job_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Interrupted);
    assert!(!report.final_submitted);
    assert!(report.provider_stream_session_ids.is_empty());
    assert!(report.tool_results.is_empty());
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn deterministic_runtime_finishes_code_task_through_tool_loop() {
    let workspace = temp_workspace("runtime-code-task");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Change answer".to_string(),
            description: "Change answer from one to two".to_string(),
            acceptance_criteria: vec!["test command passes".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Apply code edit".to_string(),
            description: "Change answer from one to two".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![
                EvidenceType::SourceRef,
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Change answer from one to two".to_string(),
            success_criteria: vec!["test command passes".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let current_exe = env::current_exe().unwrap();
    let script = DeterministicModelClient::new(vec![
        DeterministicStep::ToolCall(ToolAction::new(
            "read_file",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs"
            }),
            1,
            Some("target file was inspected before edit".to_string()),
        )),
        DeterministicStep::ToolCall(ToolAction::new(
            "apply_patch",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn answer() -> i32 { 1 }\n+pub fn answer() -> i32 { 2 }\n*** End Patch\n"
            }),
            4,
            Some("exact repository edit was applied".to_string()),
        )),
        DeterministicStep::ToolCall(ToolAction::new(
            "run_command",
            json!({
                "mode": "exec",
                "command": current_exe.to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace.to_string_lossy()
            }),
            4,
            Some("test command was run after edit".to_string()),
        )),
        DeterministicStep::Final {
            summary: "Applied exact edit and verified it.".to_string(),
            known_risks: Vec::new(),
            tests_run: vec!["test binary --help".to_string()],
            tests_not_run: Vec::new(),
        },
    ]);
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, script);
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(report.tool_results.len(), 4);
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "runtime_feedback"
            && result
                .output
                .as_ref()
                .and_then(|output| output.get("message"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|message| {
                    message.contains("patch plus command evidence already exist")
                })
    }));
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    let replayed = Kernel::from_events(&kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(replayed_state.final_submissions.len(), 1);
    assert_eq!(replayed_state.artifacts.len(), 1);
    assert!(replayed_state
        .tool_invocations
        .values()
        .any(|invocation| invocation.status == ToolCallStatus::Proposed));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_rejects_read_image_when_routed_model_lacks_image_input() {
    struct AttemptsUnsupportedReadImage {
        calls: u8,
        observed_failure: Arc<AtomicBool>,
    }

    impl ModelClient for AttemptsUnsupportedReadImage {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            if self.calls == 0 {
                self.calls += 1;
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "read_image",
                        json!({"path": "image_probe.png"}),
                        1,
                        Some("image read attempt should be rejected".to_string()),
                    ),
                )));
            }

            let image_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "read_image")
                .unwrap_or_else(|| panic!("missing read_image capability failure"));
            assert_eq!(image_result.status, ToolCallStatus::Failed);
            assert_eq!(
                image_result
                    .output
                    .as_ref()
                    .and_then(|output| output.get("stage"))
                    .and_then(serde_json::Value::as_str),
                Some("model_capability")
            );
            assert_eq!(
                image_result
                    .output
                    .as_ref()
                    .and_then(|output| output.get("error"))
                    .and_then(serde_json::Value::as_str),
                Some("read_image requires a model with image_input capability")
            );
            assert!(image_result.evidence_ids.is_empty());
            self.observed_failure.store(true, Ordering::SeqCst);
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Text-only image request rejected".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: Vec::new(),
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = temp_workspace("runtime-read-image-text-only");
    fs::write(
        workspace.join("image_probe.png"),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    )
    .unwrap();
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Handle text-only image request");
    let observed_failure = Arc::new(AtomicBool::new(false));
    let client = AttemptsUnsupportedReadImage {
        calls: 0,
        observed_failure: observed_failure.clone(),
    };
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, client);
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.requested_model_alias = Some("text-only".to_string());
    config.max_steps = 4;
    let error = runtime.run_to_completion(config).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("final answer without evidence map is rejected"),
        "unexpected runtime error: {error}"
    );
    assert!(observed_failure.load(Ordering::SeqCst));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_rejects_non_visible_tool_call_without_broker_invocation() {
    struct AttemptsDeferredTool {
        calls: u32,
    }

    impl ModelClient for AttemptsDeferredTool {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls += 1;
            let read_result = request.context.tool_results.iter().find(|result| {
                result.tool_name == "read_file" && result.status == ToolCallStatus::Completed
            });
            if let Some(read_result) = read_result {
                return Ok(ModelTurnResponse::single(ModelAction::Final {
                    submission: FinalSubmission {
                        summary: "Recovered after invisible tool rejection.".to_string(),
                        changed_artifacts: Vec::new(),
                        evidence_map: vec![EvidenceMapEntry {
                            claim: "visible read_file recovered after invisible tool rejection"
                                .to_string(),
                            evidence_refs: read_result.evidence_ids.clone(),
                        }],
                        unverified_claims: Vec::new(),
                        known_risks: Vec::new(),
                        tests_run: Vec::new(),
                        tests_not_run: Vec::new(),
                        approvals: Vec::new(),
                    },
                }));
            }
            let feedback = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == RUNTIME_FEEDBACK_TOOL);
            if let Some(feedback) = feedback {
                assert_eq!(feedback.status, ToolCallStatus::Denied);
                assert_eq!(
                    feedback
                        .output
                        .as_ref()
                        .and_then(|output| output.get("stage"))
                        .and_then(serde_json::Value::as_str),
                    Some("tool_visibility")
                );
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "read_file",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "path": "README.md"
                        }),
                        1,
                        Some(
                            "visible read_file recovered after invisible tool rejection"
                                .to_string(),
                        ),
                    ),
                )));
            }
            assert!(request
                .context
                .tool_descriptors
                .iter()
                .any(|descriptor| descriptor.name == "tool_search"));
            assert!(!request
                .context
                .tool_descriptors
                .iter()
                .any(|descriptor| descriptor.name == "mcp__echo__echo"));
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "mcp__echo__echo",
                    json!({"text": "hidden"}),
                    3,
                    Some("hidden MCP echo was attempted".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-hidden-tool");
    fs::write(workspace.join("README.md"), "visible recovery\n").unwrap();
    let kernel = Kernel::new();
    kernel
        .register_tool_descriptor(ToolDescriptor {
            tool_id: "tool_mcp__echo__echo".to_string(),
            name: "mcp__echo__echo".to_string(),
            description: "Echo one text field through MCP.".to_string(),
            version: "0.3.0".to_string(),
            driver_class: ToolDriverClass::Mcp,
            risk_level: 3,
            input_schema: json!({
                "type": "object",
                "required": ["text"],
                "properties": {"text": {"type": "string"}},
                "additionalProperties": false
            }),
            model_input_schema: Some(json!({
                "type": "object",
                "required": ["text"],
                "properties": {"text": {"type": "string"}},
                "additionalProperties": false
            })),
            output_schema: json!({"type": "object"}),
            runtime_input_policy: ToolRuntimeInputPolicy {
                required_resource_scopes: vec!["mcp:echo:echo".to_string()],
                ..ToolRuntimeInputPolicy::default()
            },
            idempotency: IdempotencyMode::ToolNative,
            evidence_type: Some(EvidenceType::ExternalReference),
            created_at: now_rfc3339(),
            ..ToolDescriptor::default()
        })
        .unwrap();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Reject invisible tool");
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        AttemptsDeferredTool { calls: 0 },
    );

    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == RUNTIME_FEEDBACK_TOOL && result.status == ToolCallStatus::Denied
    }));
    assert!(!report
        .tool_results
        .iter()
        .any(|result| result.tool_name == "mcp__echo__echo"));
    assert!(!kernel.events().unwrap().iter().any(|event| {
        event.event_type == "ToolCallProposed"
            && event.payload["tool"]["tool_name"] == json!("mcp__echo__echo")
    }));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_exposes_deferred_tool_after_tool_search_match() {
    struct SearchesThenChecksDeferredTool {
        calls: u32,
    }

    impl ModelClient for SearchesThenChecksDeferredTool {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls += 1;
            if self.calls == 1 {
                assert!(request
                    .context
                    .tool_descriptors
                    .iter()
                    .any(|descriptor| descriptor.name == "tool_search"));
                assert!(!request
                    .context
                    .tool_descriptors
                    .iter()
                    .any(|descriptor| descriptor.name == "mcp__echo__echo"));
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "tool_search",
                        json!({"query": "echo", "limit": 5}),
                        1,
                        Some("deferred MCP echo was discovered".to_string()),
                    ),
                )));
            }

            let search_result = request
                .context
                .tool_results
                .iter()
                .find(|result| {
                    result.tool_name == "tool_search" && result.status == ToolCallStatus::Completed
                })
                .ok_or_else(|| {
                    AgentOsError::Validation("tool_search result missing".to_string())
                })?;
            assert!(request
                .context
                .tool_descriptors
                .iter()
                .any(|descriptor| descriptor.name == "mcp__echo__echo"));
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Deferred MCP tool became visible after tool_search.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "tool_search exposed deferred MCP echo".to_string(),
                        evidence_refs: search_result.evidence_ids.clone(),
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = temp_workspace("runtime-tool-search-exposes-deferred");
    let kernel = Kernel::new();
    kernel
        .register_tool_descriptor(ToolDescriptor {
            tool_id: "tool_mcp__echo__echo".to_string(),
            name: "mcp__echo__echo".to_string(),
            description: "Echo one text field through MCP.".to_string(),
            version: "0.3.0".to_string(),
            driver_class: ToolDriverClass::Mcp,
            risk_level: 3,
            input_schema: json!({
                "type": "object",
                "required": ["text"],
                "properties": {"text": {"type": "string"}},
                "additionalProperties": false
            }),
            model_input_schema: Some(json!({
                "type": "object",
                "required": ["text"],
                "properties": {"text": {"type": "string"}},
                "additionalProperties": false
            })),
            output_schema: json!({"type": "object"}),
            runtime_input_policy: ToolRuntimeInputPolicy {
                required_resource_scopes: vec!["mcp:echo:echo".to_string()],
                ..ToolRuntimeInputPolicy::default()
            },
            idempotency: IdempotencyMode::ToolNative,
            evidence_type: Some(EvidenceType::ExternalReference),
            created_at: now_rfc3339(),
            ..ToolDescriptor::default()
        })
        .unwrap();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Expose deferred tool after search");
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        SearchesThenChecksDeferredTool { calls: 0 },
    );

    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "tool_search" && result.status == ToolCallStatus::Completed
    }));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_continues_model_loop_for_running_piped_process_stdin() {
    struct ContinuePipedProcess {
        command: String,
    }

    impl ModelClient for ContinuePipedProcess {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let run_record = request.context.tool_results.iter().find(|result| {
                result.tool_name == "run_command"
                    && result
                        .input
                        .as_ref()
                        .and_then(|input| input.get("command"))
                        .and_then(Value::as_str)
                        == Some(self.command.as_str())
            });
            let Some(run_record) = run_record else {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "run_command",
                        json!({
                            "command": self.command,
                            "stdin": "piped",
                            "cwd": request.workspace_root.to_string_lossy()
                        }),
                        4,
                        Some("started piped stdin process".to_string()),
                    ),
                )));
            };
            assert_eq!(run_record.status, ToolCallStatus::Running);
            let process_id = run_record
                .output
                .as_ref()
                .and_then(|output| output.get("process_id"))
                .and_then(Value::as_str)
                .expect("running run_command output should include process_id")
                .to_string();
            assert_eq!(
                run_record
                    .output
                    .as_ref()
                    .and_then(|output| output.get("stdin_mode"))
                    .and_then(Value::as_str),
                Some("piped")
            );

            let wrote_stdin = request.context.tool_results.iter().any(|result| {
                result.tool_name == "write_stdin"
                    && result
                        .input
                        .as_ref()
                        .and_then(|input| input.get("write_id"))
                        .and_then(Value::as_str)
                        == Some("runtime-stdin-1")
            });
            if !wrote_stdin {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "write_stdin",
                        json!({
                            "process_id": process_id,
                            "write_id": "runtime-stdin-1",
                            "text": "MODEL_STDIN_OK\n"
                        }),
                        4,
                        Some("wrote stdin to running process".to_string()),
                    ),
                )));
            }

            let stdout = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "write_stdin")
                .filter_map(|result| result.output.as_ref())
                .filter_map(|output| output.pointer("/process_output/chunks"))
                .filter_map(Value::as_array)
                .flat_map(|chunks| chunks.iter())
                .filter_map(|chunk| chunk.get("text"))
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("");
            if !stdout.contains("STDIN_ECHO:MODEL_STDIN_OK") {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "write_stdin",
                        json!({
                            "process_id": process_id,
                            "field": "stdout",
                            "after_sequence": {"stdout": 0}
                        }),
                        4,
                        Some("polled stdout from stdin process".to_string()),
                    ),
                )));
            }

            let evidence_map = request
                .context
                .tool_results
                .iter()
                .filter(|result| !result.evidence_ids.is_empty())
                .map(|result| EvidenceMapEntry {
                    claim: result
                        .evidence_claim
                        .clone()
                        .unwrap_or_else(|| "stdin process evidence".to_string()),
                    evidence_refs: result.evidence_ids.clone(),
                })
                .collect();
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Submitted after stdin continuation.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map,
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: vec!["write_stdin stdout poll".to_string()],
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = temp_workspace("runtime-piped-process-stdin");
    let command = if cfg!(windows) {
        "$line = [Console]::In.ReadLine(); Write-Output ('STDIN_ECHO:' + $line); Start-Sleep -Seconds 2"
    } else {
        "IFS= read -r line; printf 'STDIN_ECHO:%s\\n' \"$line\"; sleep 2"
    };
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(
        &kernel,
        &workspace,
        "Continue a running piped stdin process",
    );
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 6;
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        ContinuePipedProcess {
            command: command.to_string(),
        },
    );

    let report = runtime.run_to_completion(config).unwrap();

    assert!(
        report.final_submitted,
        "stdin continuation report: {report:#?}"
    );
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "run_command" && result.status == ToolCallStatus::Running
    }));
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "write_stdin" && result.status == ToolCallStatus::Completed
    }));
    assert_eq!(report.status, ThreadStatus::Completed);
    let _ = fs::remove_dir_all(workspace);
}

fn temp_workspace(label: &str) -> std::path::PathBuf {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-{label}-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    workspace
}

fn spawn_runtime_agent(
    kernel: &Kernel,
    workspace: &std::path::Path,
    goal_text: &str,
) -> AgentControlBlock {
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: goal_text.to_string(),
            description: goal_text.to_string(),
            acceptance_criteria: vec!["runtime completes".to_string()],
            constraints: Vec::new(),
            risk_level: 0,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: goal_text.to_string(),
            description: goal_text.to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 0,
        })
        .unwrap();
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: goal_text.to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap()
}

#[test]
fn runtime_projects_feedback_after_text_only_model_response() {
    struct TextOnlyThenFinal {
        calls: u32,
    }

    impl ModelClient for TextOnlyThenFinal {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls += 1;
            if self.calls == 1 {
                return Ok(ModelTurnResponse {
                    actions: vec![ModelAction::OutputText {
                        text: "I will inspect the repository first.".to_string(),
                    }],
                    usage: ProviderUsage::default(),
                });
            }
            let feedback = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "runtime_feedback")
                .ok_or_else(|| {
                    AgentOsError::Validation("runtime feedback was not projected".to_string())
                })?;
            assert_eq!(feedback.status, ToolCallStatus::Failed);
            let message = feedback
                .output
                .as_ref()
                .and_then(|output| output.get("message"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert!(message.contains("no tool call or final submission"));
            if request
                .context
                .tool_results
                .iter()
                .all(|result| result.tool_name != "record_evidence")
            {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "record_evidence",
                        json!({
                            "evidence_type": "runtime_trace",
                            "claim": "runtime feedback was projected after a text-only model response"
                        }),
                        2,
                        Some("runtime feedback projection was recorded".to_string()),
                    ),
                )));
            }
            let evidence_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "record_evidence")
                .unwrap();
            let evidence_id = evidence_result
                .output
                .as_ref()
                .and_then(|output| output.get("evidence_id"))
                .and_then(serde_json::Value::as_str)
                .unwrap();
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Submitted after runtime feedback.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "runtime feedback was projected after a text-only model response"
                            .to_string(),
                        evidence_refs: vec![evidence_id.to_string()],
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-feedback-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-feedback-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Handle text-only response".to_string(),
            description: "Project feedback when the model emits no action".to_string(),
            acceptance_criteria: vec!["final follows runtime feedback".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Handle text-only response".to_string(),
            description: "Handle text-only response".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Finish after feedback".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        TextOnlyThenFinal { calls: 0 },
    );
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 2);
    assert_eq!(report.tool_results[0].tool_name, "runtime_feedback");
    assert_eq!(kernel.state_snapshot().unwrap().final_submissions.len(), 1);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_feedback_after_repeated_identical_tool_call() {
    struct RepeatCommandThenFinal {
        program: std::path::PathBuf,
        workspace: std::path::PathBuf,
    }

    impl ModelClient for RepeatCommandThenFinal {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            if request
                .context
                .tool_results
                .iter()
                .any(|result| result.tool_name == "runtime_feedback")
            {
                let evidence_map = request
                    .context
                    .tool_results
                    .iter()
                    .filter(|result| !result.evidence_ids.is_empty())
                    .map(|result| EvidenceMapEntry {
                        claim: result
                            .evidence_claim
                            .clone()
                            .unwrap_or_else(|| "command evidence".to_string()),
                        evidence_refs: result.evidence_ids.clone(),
                    })
                    .collect();
                return Ok(ModelTurnResponse::single(ModelAction::Final {
                    submission: FinalSubmission {
                        summary: "Submitted after duplicate tool feedback.".to_string(),
                        changed_artifacts: Vec::new(),
                        evidence_map,
                        unverified_claims: Vec::new(),
                        known_risks: Vec::new(),
                        tests_run: Vec::new(),
                        tests_not_run: Vec::new(),
                        approvals: Vec::new(),
                    },
                }));
            }
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "run_command",
                    json!({
                        "mode": "exec",
                        "command": self.program.to_string_lossy(),
                        "args": ["--help"],
                        "cwd": self.workspace.to_string_lossy()
                    }),
                    4,
                    Some("same command was requested".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-duplicate-tool-feedback");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Handle duplicate tool calls");
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        RepeatCommandThenFinal {
            program: env::current_exe().unwrap(),
            workspace: workspace.clone(),
        },
    );

    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert!(report.final_submitted);
    let run_command_results = report
        .tool_results
        .iter()
        .filter(|result| result.tool_name == "run_command")
        .count();
    assert_eq!(run_command_results, 1);
    let feedback = report
        .tool_results
        .iter()
        .find(|result| result.tool_name == "runtime_feedback")
        .expect("duplicate tool feedback should be projected");
    let message = feedback
        .output
        .as_ref()
        .and_then(|output| output.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(message.contains("repeated an identical tool call"));
    assert!(message.contains("rejected this duplicate without executing it"));
    assert_eq!(
        feedback
            .input
            .as_ref()
            .and_then(|input| input.get("consecutive_identical_tool_calls"))
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        feedback
            .output
            .as_ref()
            .and_then(|output| output.get("severity"))
            .and_then(serde_json::Value::as_str),
        Some("warning")
    );
    let persisted_run_commands = kernel
        .state_snapshot()
        .unwrap()
        .tool_invocations
        .values()
        .filter(|invocation| {
            invocation.tool_name == "run_command" && invocation.status == ToolCallStatus::Completed
        })
        .count();
    assert_eq!(persisted_run_commands, 1);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_feedback_after_repeated_identical_read_file_call() {
    struct RepeatReadThenFinal;

    impl ModelClient for RepeatReadThenFinal {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            if request
                .context
                .tool_results
                .iter()
                .any(|result| result.tool_name == "runtime_feedback")
            {
                let evidence_map = request
                    .context
                    .tool_results
                    .iter()
                    .filter(|result| result.tool_name == "read_file")
                    .filter_map(|result| {
                        let claim = result.evidence_claim.clone()?;
                        Some(EvidenceMapEntry {
                            claim,
                            evidence_refs: result.evidence_ids.clone(),
                        })
                    })
                    .collect();
                return Ok(ModelTurnResponse::single(ModelAction::Final {
                    submission: FinalSubmission {
                        summary: "Submitted after duplicate read feedback.".to_string(),
                        changed_artifacts: Vec::new(),
                        evidence_map,
                        unverified_claims: Vec::new(),
                        known_risks: Vec::new(),
                        tests_run: Vec::new(),
                        tests_not_run: Vec::new(),
                        approvals: Vec::new(),
                    },
                }));
            }
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "read_file",
                    json!({
                        "workspace_root": request.workspace_root.to_string_lossy(),
                        "path": "README.md"
                    }),
                    1,
                    Some("README was inspected".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-duplicate-read-feedback");
    fs::write(workspace.join("README.md"), "hello\n").unwrap();
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Handle duplicate read calls");
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 5;
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, RepeatReadThenFinal);

    let report = runtime.run_to_completion(config).unwrap();

    assert!(report.final_submitted);
    assert_eq!(
        report
            .tool_results
            .iter()
            .filter(|result| result.tool_name == "read_file")
            .count(),
        1
    );
    let feedback = report
        .tool_results
        .iter()
        .find(|result| result.tool_name == "runtime_feedback")
        .expect("duplicate read feedback should be projected");
    assert_eq!(
        feedback
            .input
            .as_ref()
            .and_then(|input| input.get("tool_name"))
            .and_then(serde_json::Value::as_str),
        Some("read_file")
    );
    assert_eq!(
        feedback
            .output
            .as_ref()
            .and_then(|output| output.get("severity"))
            .and_then(serde_json::Value::as_str),
        Some("warning")
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_blocks_on_fifth_consecutive_identical_tool_call() {
    struct RepeatReadForever;

    impl ModelClient for RepeatReadForever {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "read_file",
                    json!({
                        "workspace_root": request.workspace_root.to_string_lossy(),
                        "path": "README.md"
                    }),
                    1,
                    Some("README was inspected".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-duplicate-tool-block");
    fs::write(workspace.join("README.md"), "hello\n").unwrap();
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Block duplicate read calls");
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 10;
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, RepeatReadForever);

    let report = runtime.run_to_completion(config).unwrap();

    assert_eq!(report.status, ThreadStatus::Blocked);
    assert!(!report.final_submitted);
    assert_eq!(
        report
            .tool_results
            .iter()
            .filter(|result| result.tool_name == "read_file")
            .count(),
        1
    );
    let duplicate_feedback = report
        .tool_results
        .iter()
        .filter(|result| result.tool_name == "runtime_feedback")
        .filter(|result| {
            result
                .input
                .as_ref()
                .and_then(|input| input.get("tool_name"))
                .and_then(serde_json::Value::as_str)
                == Some("read_file")
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_feedback.len(), 4);
    let severities = duplicate_feedback
        .iter()
        .map(|result| {
            result
                .output
                .as_ref()
                .and_then(|output| output.get("severity"))
                .and_then(serde_json::Value::as_str)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(severities, vec!["warning", "warning", "warning", "error"]);
    let duplicate_counts = duplicate_feedback
        .iter()
        .map(|result| {
            result
                .input
                .as_ref()
                .and_then(|input| input.get("consecutive_identical_tool_calls"))
                .and_then(serde_json::Value::as_u64)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_counts, vec![2, 3, 4, 5]);
    assert_eq!(
        duplicate_feedback
            .last()
            .and_then(|result| result.input.as_ref())
            .and_then(|input| input.get("consecutive_identical_tool_calls"))
            .and_then(serde_json::Value::as_u64),
        Some(MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS as u64)
    );
    let state = kernel.state_snapshot().unwrap();
    let task = state.tasks.get(&report.task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert!(task
        .blocked_reason
        .as_deref()
        .unwrap_or_default()
        .contains("5 consecutive identical tool calls"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_finalization_feedback_after_patch_and_command_and_filters_tools() {
    struct FinishAfterFinalizationFeedback {
        program: std::path::PathBuf,
        workspace: std::path::PathBuf,
        attempted_extra_command_after_feedback: bool,
        saw_filtered_tools: Arc<AtomicBool>,
    }

    impl ModelClient for FinishAfterFinalizationFeedback {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let has_gate_feedback = request.context.tool_results.iter().any(|result| {
                result.tool_name == "runtime_feedback"
                    && result
                        .output
                        .as_ref()
                        .and_then(|output| output.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|message| message.contains("finalization gate is active"))
            });
            if request.context.tool_results.iter().any(|result| {
                result.tool_name == "runtime_feedback"
                    && result
                        .output
                        .as_ref()
                        .and_then(|output| output.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|message| {
                            message.contains("patch plus command evidence already exist")
                        })
            }) {
                let visible_tool_names = request
                    .context
                    .tool_descriptors
                    .iter()
                    .map(|descriptor| descriptor.name.as_str())
                    .collect::<Vec<_>>();
                assert!(visible_tool_names.contains(&"submit_final"));
                assert!(visible_tool_names.contains(&"accomplish_goal"));
                assert!(!visible_tool_names.contains(&"apply_patch"));
                assert!(!visible_tool_names.contains(&"run_command"));
                assert!(!visible_tool_names.contains(&"read_file"));
                self.saw_filtered_tools.store(true, Ordering::SeqCst);
                if !has_gate_feedback && !self.attempted_extra_command_after_feedback {
                    self.attempted_extra_command_after_feedback = true;
                    return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "run_command",
                            json!({
                                "mode": "exec",
                                "command": self.program.to_string_lossy(),
                                "args": ["--help"],
                                "cwd": self.workspace.to_string_lossy()
                            }),
                            4,
                            Some("extra command after finalization feedback".to_string()),
                        ),
                    )));
                }
                let evidence_map = request
                    .context
                    .tool_results
                    .iter()
                    .filter(|result| !result.evidence_ids.is_empty())
                    .map(|result| EvidenceMapEntry {
                        claim: result
                            .evidence_claim
                            .clone()
                            .unwrap_or_else(|| "runtime evidence".to_string()),
                        evidence_refs: result.evidence_ids.clone(),
                    })
                    .collect();
                return Ok(ModelTurnResponse::single(ModelAction::Final {
                    submission: FinalSubmission {
                        summary: "Submitted after finalization feedback.".to_string(),
                        changed_artifacts: request
                            .context
                            .artifacts
                            .iter()
                            .map(|artifact| artifact.artifact_id.clone())
                            .collect(),
                        evidence_map,
                        unverified_claims: Vec::new(),
                        known_risks: Vec::new(),
                        tests_run: vec!["test binary --help".to_string()],
                        tests_not_run: Vec::new(),
                        approvals: Vec::new(),
                    },
                }));
            }
            if request.context.artifacts.is_empty() {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "apply_patch",
                        json!({
                            "workspace_root": self.workspace.to_string_lossy(),
                            "patch": "*** Begin Patch\n*** Add File: result.txt\n+done\n*** End Patch\n"
                        }),
                        4,
                        Some("patch was applied".to_string()),
                    ),
                )));
            }
            if request
                .context
                .tool_results
                .iter()
                .all(|result| result.tool_name != "run_command")
            {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "run_command",
                        json!({
                            "mode": "exec",
                            "command": self.program.to_string_lossy(),
                            "args": ["--help"],
                            "cwd": self.workspace.to_string_lossy()
                        }),
                        4,
                        Some("validation command was captured".to_string()),
                    ),
                )));
            }
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "read_file",
                    json!({
                        "workspace_root": self.workspace.to_string_lossy(),
                        "path": "result.txt"
                    }),
                    1,
                    Some("model kept inspecting after validation".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-finalization-feedback");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Finalize after validation");
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 24;
    let saw_filtered_tools = Arc::new(AtomicBool::new(false));
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        FinishAfterFinalizationFeedback {
            program: env::current_exe().unwrap(),
            workspace: workspace.clone(),
            attempted_extra_command_after_feedback: false,
            saw_filtered_tools: saw_filtered_tools.clone(),
        },
    );

    let report = runtime.run_to_completion(config).unwrap();

    assert!(report.final_submitted);
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "runtime_feedback"
            && result
                .output
                .as_ref()
                .and_then(|output| output.get("message"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|message| {
                    message.contains("patch plus command evidence already exist")
                })
    }));
    assert!(report.tool_results.iter().any(|result| {
        result.tool_name == "runtime_feedback"
            && result
                .output
                .as_ref()
                .and_then(|output| output.get("message"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|message| message.contains("finalization gate is active"))
    }));
    assert!(saw_filtered_tools.load(Ordering::SeqCst));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn finalization_feedback_requires_successful_post_patch_command_output() {
    fn tool_result(tool_name: &str, output: serde_json::Value) -> ToolExecutionRecord {
        ToolExecutionRecord {
            call_id: format!("{tool_name}_call"),
            tool_name: tool_name.to_string(),
            status: ToolCallStatus::Completed,
            input: None,
            output: Some(output),
            evidence_ids: vec![format!("{tool_name}_evidence")],
            evidence_claim: Some(format!("{tool_name} evidence")),
        }
    }

    let artifacts = vec![ArtifactRecord {
        artifact_id: "artifact_patch".to_string(),
        artifact_type: ArtifactType::Patch,
        blob_ref: None,
        evidence_ids: vec!["patch_evidence".to_string()],
    }];
    let patch = tool_result("apply_patch", json!({"status": "completed"}));

    let failed_command = tool_result("run_command", json!({"exit_code": 1}));
    assert!(!super::feedback::should_project_finalization_feedback(
        &[patch.clone(), failed_command],
        &artifacts
    ));

    let missing_exit_code_command = tool_result("run_command", json!({"status": "completed"}));
    assert!(!super::feedback::should_project_finalization_feedback(
        &[patch.clone(), missing_exit_code_command],
        &artifacts
    ));

    let silent_successful_command = tool_result("run_command", json!({"exit_code": 0}));
    assert!(!super::feedback::should_project_finalization_feedback(
        &[patch.clone(), silent_successful_command],
        &artifacts
    ));

    let successful_command = tool_result(
        "run_command",
        json!({"exit_code": 0, "stdout_bytes": 9, "stderr_bytes": 0}),
    );
    assert!(super::feedback::should_project_finalization_feedback(
        &[patch, successful_command],
        &artifacts
    ));
}

#[test]
fn runtime_projects_pre_patch_resolution_feedback_after_bounded_investigation() {
    struct ResolveAfterPrePatchResolutionFeedback {
        program: std::path::PathBuf,
        workspace: std::path::PathBuf,
        investigation_calls: usize,
        attempted_extra_command_after_feedback: bool,
        saw_filtered_tools: Arc<AtomicBool>,
        saw_gate_rejection: Arc<AtomicBool>,
    }

    impl ModelClient for ResolveAfterPrePatchResolutionFeedback {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let has_patch = request.context.tool_results.iter().any(|result| {
                result.tool_name == "apply_patch" && result.status == ToolCallStatus::Completed
            });
            let has_post_patch_command = request.context.tool_results.iter().any(|result| {
                result.tool_name == "run_command"
                    && result.status == ToolCallStatus::Completed
                    && !result.evidence_ids.is_empty()
            });
            if has_patch && has_post_patch_command {
                let evidence_map = request
                    .context
                    .tool_results
                    .iter()
                    .filter(|result| !result.evidence_ids.is_empty())
                    .map(|result| EvidenceMapEntry {
                        claim: result
                            .evidence_claim
                            .clone()
                            .unwrap_or_else(|| "runtime evidence".to_string()),
                        evidence_refs: result.evidence_ids.clone(),
                    })
                    .collect();
                return Ok(ModelTurnResponse::single(ModelAction::Final {
                    submission: FinalSubmission {
                        summary: "Submitted after pre-patch resolution gate.".to_string(),
                        changed_artifacts: request
                            .context
                            .artifacts
                            .iter()
                            .map(|artifact| artifact.artifact_id.clone())
                            .collect(),
                        evidence_map,
                        unverified_claims: Vec::new(),
                        known_risks: Vec::new(),
                        tests_run: vec!["test binary --help".to_string()],
                        tests_not_run: Vec::new(),
                        approvals: Vec::new(),
                    },
                }));
            }
            if has_patch {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "run_command",
                        json!({
                            "mode": "exec",
                            "command": self.program.to_string_lossy(),
                            "args": ["--help"],
                            "cwd": self.workspace.to_string_lossy()
                        }),
                        4,
                        Some("post-patch command evidence was captured".to_string()),
                    ),
                )));
            }

            let has_resolution_feedback = request.context.tool_results.iter().any(|result| {
                result.tool_name == "runtime_feedback"
                    && result
                        .output
                        .as_ref()
                        .and_then(|output| output.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|message| {
                            message.contains("pre-patch investigation budget is nearly exhausted")
                        })
            });
            let has_gate_rejection = request.context.tool_results.iter().any(|result| {
                result.tool_name == "runtime_feedback"
                    && result
                        .output
                        .as_ref()
                        .and_then(|output| output.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|message| {
                            message.contains("pre-patch resolution gate is active")
                        })
            });
            if has_resolution_feedback {
                let visible_tool_names = request
                    .context
                    .tool_descriptors
                    .iter()
                    .map(|descriptor| descriptor.name.as_str())
                    .collect::<Vec<_>>();
                assert!(visible_tool_names.contains(&"apply_patch"));
                assert!(visible_tool_names.contains(&"submit_final"));
                assert!(visible_tool_names.contains(&"accomplish_goal"));
                if self.investigation_calls < PRE_PATCH_HARD_GATE_TOOL_RESULTS {
                    assert!(visible_tool_names.contains(&"read_file"));
                    assert!(visible_tool_names.contains(&"run_command"));
                    self.investigation_calls += 1;
                    return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "read_file",
                            json!({
                                "workspace_root": self.workspace.to_string_lossy(),
                                "path": "notes.txt",
                                "offset": 1,
                                "limit": self.investigation_calls,
                            }),
                            1,
                            Some("pre-patch investigation read after soft feedback".to_string()),
                        ),
                    )));
                }
                assert!(!visible_tool_names.contains(&"read_file"));
                assert!(!visible_tool_names.contains(&"run_command"));
                self.saw_filtered_tools.store(true, Ordering::SeqCst);
                if !has_gate_rejection && !self.attempted_extra_command_after_feedback {
                    self.attempted_extra_command_after_feedback = true;
                    return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "run_command",
                            json!({
                                "mode": "exec",
                                "command": self.program.to_string_lossy(),
                                "args": ["--help"],
                                "cwd": self.workspace.to_string_lossy()
                            }),
                            4,
                            Some("extra command after pre-patch feedback".to_string()),
                        ),
                    )));
                }
                self.saw_gate_rejection.store(true, Ordering::SeqCst);
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "apply_patch",
                        json!({
                            "workspace_root": self.workspace.to_string_lossy(),
                            "patch": "*** Begin Patch\n*** Update File: notes.txt\n@@\n-line 1\n+line 1 patched\n*** End Patch\n"
                        }),
                        4,
                        Some("notes.txt was patched after bounded investigation".to_string()),
                    ),
                )));
            }

            self.investigation_calls += 1;
            Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                ToolAction::new(
                    "read_file",
                    json!({
                        "workspace_root": self.workspace.to_string_lossy(),
                        "path": "notes.txt",
                        "offset": 1,
                        "limit": self.investigation_calls,
                    }),
                    1,
                    Some("pre-patch investigation read".to_string()),
                ),
            )))
        }
    }

    let workspace = temp_workspace("runtime-pre-patch-resolution");
    fs::write(
        workspace.join("notes.txt"),
        (1..=20)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Resolve after bounded investigation");
    let saw_filtered_tools = Arc::new(AtomicBool::new(false));
    let saw_gate_rejection = Arc::new(AtomicBool::new(false));
    let script = ResolveAfterPrePatchResolutionFeedback {
        program: env::current_exe().unwrap(),
        workspace: workspace.clone(),
        investigation_calls: 0,
        attempted_extra_command_after_feedback: false,
        saw_filtered_tools: saw_filtered_tools.clone(),
        saw_gate_rejection: saw_gate_rejection.clone(),
    };
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 48;
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), script);

    let report = runtime.run_to_completion(config).unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert!(saw_filtered_tools.load(Ordering::SeqCst));
    assert!(saw_gate_rejection.load(Ordering::SeqCst));
    assert!(fs::read_to_string(workspace.join("notes.txt"))
        .unwrap()
        .starts_with("line 1 patched\n"));
    let feedback_messages = report
        .tool_results
        .iter()
        .filter(|result| result.tool_name == "runtime_feedback")
        .filter_map(|result| {
            result
                .output
                .as_ref()
                .and_then(|output| output.get("message"))
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Vec<_>>();
    assert!(feedback_messages
        .iter()
        .any(|message| message.contains("pre-patch investigation budget is nearly exhausted")));
    assert!(feedback_messages
        .iter()
        .any(|message| message.contains("pre-patch resolution gate is active")));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_returns_blocked_report_after_consecutive_no_action_model_responses() {
    struct TextOnly;

    impl ModelClient for TextOnly {
        fn next(&mut self, _request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            Ok(ModelTurnResponse {
                actions: vec![ModelAction::OutputText {
                    text: "(no action from model)".to_string(),
                }],
                usage: ProviderUsage::default(),
            })
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-no-action-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-no-action-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Block repeated no-action turns".to_string(),
            description: "Repeated no-action turns must stop the runtime without process failure"
                .to_string(),
            acceptance_criteria: vec!["runtime returns a blocked report".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Block repeated no-action turns".to_string(),
            description: "Block repeated no-action turns".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let task_id = task.task_id.clone();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Block after repeated no-action responses".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, TextOnly);
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Blocked);
    assert!(!report.final_submitted);
    let completed_streams = kernel
        .events()
        .unwrap()
        .iter()
        .filter(|event| event.event_type == "ProviderStreamCompleted")
        .count();
    assert_eq!(completed_streams, 2);
    let state = kernel.state_snapshot().unwrap();
    assert_eq!(state.final_submissions.len(), 0);
    let task = state.tasks.get(&task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert!(task
        .blocked_reason
        .as_deref()
        .unwrap_or_default()
        .contains("consecutive model turns with no tool call"));
    let thread = state.threads.get(&report.thread_id).unwrap();
    assert_eq!(thread.active_turn.status, Some(TurnStatus::Blocked));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_returns_blocked_report_at_max_steps_without_final_submission() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-max-steps-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("result.txt"), "before\n").unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-max-steps-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Stop at step limit".to_string(),
            description: "Step limit is a noncompletion report".to_string(),
            acceptance_criteria: vec!["runtime returns a blocked report".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Stop at step limit".to_string(),
            description: "Stop at step limit".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let task_id = task.task_id.clone();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Make one edit but do not submit final".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let script = DeterministicModelClient::new(vec![DeterministicStep::ToolCall(ToolAction::new(
        "apply_patch",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "patch": "*** Begin Patch\n*** Update File: result.txt\n@@\n-before\n+after\n*** End Patch\n"
        }),
        4,
        Some("result file was edited before the step limit".to_string()),
    ))]);
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 1;
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id, script);

    let report = runtime.run_to_completion(config).unwrap();

    assert_eq!(report.status, ThreadStatus::Blocked);
    assert!(!report.final_submitted);
    assert_eq!(
        fs::read_to_string(workspace.join("result.txt")).unwrap(),
        "after\n"
    );
    let state = kernel.state_snapshot().unwrap();
    let task = state.tasks.get(&task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(
        task.blocked_reason.as_deref(),
        Some("runtime reached max_steps without final submission")
    );
    let thread = state.threads.get(&report.thread_id).unwrap();
    assert_eq!(thread.active_turn.status, Some(TurnStatus::Blocked));
    assert_eq!(state.final_submissions.len(), 0);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_compacts_older_evidence_tool_outputs_in_projection() {
    struct ReadsUntilProjectionCompacts;

    impl ModelClient for ReadsUntilProjectionCompacts {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let read_results = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "read_file")
                .collect::<Vec<_>>();
            if read_results.len() < 10 {
                let path = format!("large-{}.txt", read_results.len());
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "read_file",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "path": path
                        }),
                        1,
                        Some("large file was inspected".to_string()),
                    ),
                )));
            }
            let first_content = read_results[0]
                .output
                .as_ref()
                .and_then(|output| output.get("content"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let last_content = read_results[9]
                .output
                .as_ref()
                .and_then(|output| output.get("content"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert!(first_content.len() < 3000);
            assert!(first_content.contains("truncated for projection"));
            assert!(last_content.len() > 8000);
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Projection compacted older evidence.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "large file was inspected".to_string(),
                        evidence_refs: read_results[0].evidence_ids.clone(),
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-compact-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    for index in 0..10 {
        fs::write(
            workspace.join(format!("large-{index}.txt")),
            "x".repeat(12_000),
        )
        .unwrap();
    }
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-compact-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Compact projection".to_string(),
            description: "Compact older evidence-bearing tool outputs".to_string(),
            acceptance_criteria: vec!["older evidence output is bounded".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Compact projection".to_string(),
            description: "Compact projection".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Read large file repeatedly".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 12;
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        ReadsUntilProjectionCompacts,
    );
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_failed_tool_result_for_model_recovery() {
    struct RecoversAfterFailedReplace;

    impl ModelClient for RecoversAfterFailedReplace {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let replace_results = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "apply_patch")
                .collect::<Vec<_>>();
            match replace_results.as_slice() {
                [] => Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "apply_patch",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-missing\n+value = \"new\"\n*** End Patch\n"
                        }),
                        4,
                        Some("first edit attempt was tried".to_string()),
                    ),
                ))),
                [failed] => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    let error = failed
                        .output
                        .as_ref()
                        .and_then(|output| output.get("error"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    assert!(error.contains("apply_patch update hunk did not match file content"));
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "apply_patch",
                            json!({
                                "workspace_root": request.workspace_root.to_string_lossy(),
                                "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-value = \"old\"\n+value = \"new\"\n*** End Patch\n"
                            }),
                            4,
                            Some("corrected edit was applied after failed attempt".to_string()),
                        ),
                    )))
                }
                [failed, completed] => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    assert_eq!(completed.status, ToolCallStatus::Completed);
                    Ok(ModelTurnResponse::single(ModelAction::Final {
                        submission: FinalSubmission {
                            summary: "Recovered from failed edit and submitted final.".to_string(),
                            changed_artifacts: request
                                .context
                                .artifacts
                                .iter()
                                .map(|artifact| artifact.artifact_id.clone())
                                .collect(),
                            evidence_map: vec![EvidenceMapEntry {
                                claim: "corrected edit was applied after failed attempt"
                                    .to_string(),
                                evidence_refs: completed.evidence_ids.clone(),
                            }],
                            unverified_claims: Vec::new(),
                            known_risks: Vec::new(),
                            tests_run: Vec::new(),
                            tests_not_run: Vec::new(),
                            approvals: Vec::new(),
                        },
                    }))
                }
                _ => Err(AgentOsError::Validation(
                    "unexpected apply_patch retry count".to_string(),
                )),
            }
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-tool-failure-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(workspace.join("src/lib.rs"), "value = \"old\"\n").unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-tool-failure-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Recover edit".to_string(),
            description: "Recover after a failed exact edit".to_string(),
            acceptance_criteria: vec!["final submission follows corrected edit".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Recover edit".to_string(),
            description: "Recover after a failed exact edit".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Recover after a failed apply_patch call".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id, RecoversAfterFailedReplace);
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 2);
    assert_eq!(report.tool_results[0].status, ToolCallStatus::Failed);
    assert_eq!(report.tool_results[1].status, ToolCallStatus::Completed);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "value = \"new\"\n"
    );
    let state = kernel.state_snapshot().unwrap();
    assert!(state
        .tool_invocations
        .values()
        .any(|invocation| invocation.status == ToolCallStatus::Failed));
    assert_eq!(state.final_submissions.len(), 1);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_failed_run_command_results_for_model_recovery() {
    struct RecoversAfterFailedCommands;

    impl ModelClient for RecoversAfterFailedCommands {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let command_results = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "run_command")
                .collect::<Vec<_>>();
            let evidence_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "record_evidence");
            match (command_results.as_slice(), evidence_result) {
                ([], _) => Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "run_command",
                        json!({
                            "command": "cmd",
                            "mode": "exec",
                            "args": {"bad": "shape"},
                            "cwd": request.workspace_root.to_string_lossy()
                        }),
                        4,
                        Some("invalid command input was attempted".to_string()),
                    ),
                ))),
                ([schema_failure], _) => {
                    assert_eq!(schema_failure.status, ToolCallStatus::Failed);
                    let error = schema_failure
                        .output
                        .as_ref()
                        .and_then(|output| output.get("error"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    assert!(error.contains("tool.input.args expected array"));
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "run_command",
                            json!({
                                "mode": "exec",
                                "command": "agent-os-definitely-missing-executable",
                                "args": [],
                                "cwd": request.workspace_root.to_string_lossy()
                            }),
                            4,
                            Some("missing command was attempted".to_string()),
                        ),
                    )))
                }
                ([schema_failure, process_failure], None) => {
                    assert_eq!(schema_failure.status, ToolCallStatus::Failed);
                    assert_eq!(process_failure.status, ToolCallStatus::Failed);
                    let error = process_failure
                        .output
                        .as_ref()
                        .and_then(|output| output.get("error"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    assert!(error.contains("run process"));
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "record_evidence",
                            json!({
                                "evidence_type": "runtime_trace",
                                "claim": "failed run_command attempts were projected back to the model",
                                "metadata": {
                                    "failed_call_ids": [
                                        schema_failure.call_id,
                                        process_failure.call_id
                                    ]
                                }
                            }),
                            2,
                            Some("failed command attempts were recorded".to_string()),
                        ),
                    )))
                }
                ([schema_failure, process_failure], Some(evidence)) => {
                    assert_eq!(schema_failure.status, ToolCallStatus::Failed);
                    assert_eq!(process_failure.status, ToolCallStatus::Failed);
                    let evidence_id = evidence
                        .output
                        .as_ref()
                        .and_then(|output| output.get("evidence_id"))
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            AgentOsError::Validation(
                                "record_evidence output omitted evidence_id".to_string(),
                            )
                        })?;
                    Ok(ModelTurnResponse::single(ModelAction::Final {
                        submission: FinalSubmission {
                            summary: "Recovered from failed command attempts.".to_string(),
                            changed_artifacts: Vec::new(),
                            evidence_map: vec![EvidenceMapEntry {
                                claim:
                                    "failed run_command attempts were projected back to the model"
                                        .to_string(),
                                evidence_refs: vec![evidence_id.to_string()],
                            }],
                            unverified_claims: Vec::new(),
                            known_risks: vec![
                                "command attempts failed before verification".to_string()
                            ],
                            tests_run: Vec::new(),
                            tests_not_run: vec![
                                "command verification could not run with invalid command inputs"
                                    .to_string(),
                            ],
                            approvals: Vec::new(),
                        },
                    }))
                }
                _ => Err(AgentOsError::Validation(
                    "unexpected run_command retry count".to_string(),
                )),
            }
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-command-failure-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-command-failure-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Recover command".to_string(),
            description: "Recover after failed command attempts".to_string(),
            acceptance_criteria: vec!["final submission explains failed commands".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Recover command".to_string(),
            description: "Recover after failed command attempts".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Recover after failed run_command calls".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.fail_on_process_nonzero = true;
    let mut runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id, RecoversAfterFailedCommands);
    let report = runtime.run_to_completion(config).unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 3);
    assert_eq!(report.tool_results[0].status, ToolCallStatus::Failed);
    assert_eq!(report.tool_results[1].status, ToolCallStatus::Failed);
    assert_eq!(report.tool_results[2].status, ToolCallStatus::Completed);
    let state = kernel.state_snapshot().unwrap();
    assert_eq!(
        state
            .tool_invocations
            .values()
            .filter(|invocation| invocation.status == ToolCallStatus::Failed)
            .count(),
        2
    );
    assert_eq!(state.final_submissions.len(), 1);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_context_projection_is_scoped_to_current_task_and_thread() {
    struct AssertsScopedContext {
        calls: u8,
        current_context_id: String,
        sibling_context_id: String,
        current_compaction_id: String,
        sibling_compaction_id: String,
        current_fork_id: String,
        sibling_fork_id: String,
        current_rollback_id: String,
        sibling_rollback_id: String,
    }

    impl ModelClient for AssertsScopedContext {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            if self.calls == 0 {
                self.calls += 1;
                assert!(request
                    .context
                    .context_snapshots
                    .iter()
                    .any(|snapshot| snapshot.context_id == self.current_context_id));
                assert!(!request
                    .context
                    .context_snapshots
                    .iter()
                    .any(|snapshot| snapshot.context_id == self.sibling_context_id));
                assert!(request
                    .context
                    .context_compactions
                    .iter()
                    .any(|compaction| compaction.compaction_id == self.current_compaction_id));
                assert!(!request
                    .context
                    .context_compactions
                    .iter()
                    .any(|compaction| compaction.compaction_id == self.sibling_compaction_id));
                assert!(request
                    .context
                    .thread_forks
                    .iter()
                    .any(|fork| fork.fork_id == self.current_fork_id));
                assert!(!request
                    .context
                    .thread_forks
                    .iter()
                    .any(|fork| fork.fork_id == self.sibling_fork_id));
                assert!(request
                    .context
                    .thread_rollbacks
                    .iter()
                    .any(|rollback| rollback.rollback_id == self.current_rollback_id));
                assert!(!request
                    .context
                    .thread_rollbacks
                    .iter()
                    .any(|rollback| rollback.rollback_id == self.sibling_rollback_id));
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "record_evidence",
                        json!({
                            "evidence_type": "runtime_trace",
                            "claim": "scoped context projection was observed",
                            "metadata": {"scope": "current task and thread only"}
                        }),
                        2,
                        Some("scoped context projection was observed".to_string()),
                    ),
                )));
            }

            let evidence_id = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "record_evidence")
                .and_then(|result| result.output.as_ref())
                .and_then(|output| output.get("evidence_id"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    AgentOsError::Validation(
                        "record_evidence result missing evidence_id".to_string(),
                    )
                })?;

            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Scoped context projection verified.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "scoped context projection was observed".to_string(),
                        evidence_refs: vec![evidence_id.to_string()],
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: vec![
                        "runtime_context_projection_is_scoped_to_current_task_and_thread"
                            .to_string(),
                    ],
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = temp_workspace("runtime-context-scope");
    let kernel = Kernel::new();
    let current = spawn_runtime_agent(&kernel, &workspace, "Current scoped context");
    let sibling = spawn_runtime_agent(&kernel, &workspace, "Sibling scoped context");
    let current_context = kernel
        .load_context(LoadContextInput {
            agent_id: current.agent_id.clone(),
            task_id: current.task.task_id.clone(),
            loaded_refs: vec!["current-ref".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 64,
        })
        .unwrap();
    let sibling_context = kernel
        .load_context(LoadContextInput {
            agent_id: sibling.agent_id.clone(),
            task_id: sibling.task.task_id.clone(),
            loaded_refs: vec!["sibling-ref".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 64,
        })
        .unwrap();
    let current_compaction = kernel
        .compact_context(CompactContextInput {
            thread_id: current.thread_id.clone(),
            agent_id: current.agent_id.clone(),
            task_id: current.task.task_id.clone(),
            summary_artifact_id: None,
            superseded_refs: vec![format!("context_snapshot:{}", current_context.context_id)],
            token_estimate: 128,
        })
        .unwrap();
    let sibling_compaction = kernel
        .compact_context(CompactContextInput {
            thread_id: sibling.thread_id.clone(),
            agent_id: sibling.agent_id.clone(),
            task_id: sibling.task.task_id.clone(),
            summary_artifact_id: None,
            superseded_refs: vec![format!("context_snapshot:{}", sibling_context.context_id)],
            token_estimate: 128,
        })
        .unwrap();
    let (current_fork, _) = kernel
        .fork_thread(ForkThreadInput {
            source_thread_id: current.thread_id.clone(),
            from_turn_id: None,
            created_by_client_id: "agent-os-thread-test".to_string(),
            title: Some("current fork".to_string()),
            goal: Some("current fork".to_string()),
        })
        .unwrap();
    let (sibling_fork, _) = kernel
        .fork_thread(ForkThreadInput {
            source_thread_id: sibling.thread_id.clone(),
            from_turn_id: None,
            created_by_client_id: "agent-os-thread-test".to_string(),
            title: Some("sibling fork".to_string()),
            goal: Some("sibling fork".to_string()),
        })
        .unwrap();
    let current_thread_event_id = kernel
        .events()
        .unwrap()
        .into_iter()
        .find(|event| {
            event.aggregate_id == current.thread_id && event.event_type == "ThreadConfigured"
        })
        .unwrap()
        .event_id;
    let sibling_thread_event_id = kernel
        .events()
        .unwrap()
        .into_iter()
        .find(|event| {
            event.aggregate_id == sibling.thread_id && event.event_type == "ThreadConfigured"
        })
        .unwrap()
        .event_id;
    let (current_rollback, _) = kernel
        .rollback_thread(RollbackThreadInput {
            thread_id: current.thread_id.clone(),
            target_turn_id: None,
            target_item_id: None,
            target_event_id: Some(current_thread_event_id),
            reason: "current rollback marker".to_string(),
            created_by_client_id: "agent-os-thread-test".to_string(),
        })
        .unwrap();
    let (sibling_rollback, _) = kernel
        .rollback_thread(RollbackThreadInput {
            thread_id: sibling.thread_id.clone(),
            target_turn_id: None,
            target_item_id: None,
            target_event_id: Some(sibling_thread_event_id),
            reason: "sibling rollback marker".to_string(),
            created_by_client_id: "agent-os-thread-test".to_string(),
        })
        .unwrap();

    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        current.thread_id.clone(),
        AssertsScopedContext {
            calls: 0,
            current_context_id: current_context.context_id,
            sibling_context_id: sibling_context.context_id,
            current_compaction_id: current_compaction.compaction_id,
            sibling_compaction_id: sibling_compaction.compaction_id,
            current_fork_id: current_fork.fork_id,
            sibling_fork_id: sibling_fork.fork_id,
            current_rollback_id: current_rollback.rollback_id,
            sibling_rollback_id: sibling_rollback.rollback_id,
        },
    );

    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_projects_only_active_memory_records_into_model_context() {
    struct AssertsActiveMemoryProjection {
        calls: u8,
        active_memory_id: String,
        proposed_memory_id: String,
        invalidated_memory_id: String,
    }

    impl ModelClient for AssertsActiveMemoryProjection {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            if self.calls == 0 {
                self.calls += 1;
                assert!(request
                    .context
                    .memory_records
                    .iter()
                    .all(|record| record.status == MemoryStatus::Active));
                let active = request
                    .context
                    .memory_records
                    .iter()
                    .find(|record| record.memory_id == self.active_memory_id)
                    .expect("active memory projected into model context");
                assert_eq!(
                    active.content["decision"],
                    "ship deterministic runtime memory projection"
                );
                assert!(!request
                    .context
                    .memory_records
                    .iter()
                    .any(|record| record.memory_id == self.proposed_memory_id));
                assert!(!request
                    .context
                    .memory_records
                    .iter()
                    .any(|record| record.memory_id == self.invalidated_memory_id));
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "record_evidence",
                        json!({
                            "evidence_type": "runtime_trace",
                            "claim": "active memory projection was observed",
                            "metadata": {"memory_id": self.active_memory_id}
                        }),
                        2,
                        Some("active memory projection was observed".to_string()),
                    ),
                )));
            }

            let evidence_id = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "record_evidence")
                .and_then(|result| result.output.as_ref())
                .and_then(|output| output.get("evidence_id"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    AgentOsError::Validation(
                        "record_evidence result missing evidence_id".to_string(),
                    )
                })?;

            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Active memory projection verified.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "active memory projection was observed".to_string(),
                        evidence_refs: vec![evidence_id.to_string()],
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: vec![
                        "runtime_projects_only_active_memory_records_into_model_context"
                            .to_string(),
                    ],
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = temp_workspace("runtime-memory-projection");
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Project active memory");
    let evidence = kernel
        .attach_evidence(AttachEvidenceInput {
            goal_id: agent.task.goal_id.clone(),
            task_id: Some(agent.task.task_id.clone()),
            artifact_id: None,
            evidence_type: EvidenceType::SourceRef,
            producer_agent_id: Some(agent.agent_id.clone()),
            claim: Some("memory provenance source".to_string()),
            blob_ref: Some("memory://runtime-projection-source".to_string()),
            content_hash: None,
            inline_bytes: None,
            metadata: json!({"source": "runtime memory projection test"}),
        })
        .unwrap();
    let active_memory = kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"decision": "ship deterministic runtime memory projection"}),
            created_by_agent_id: agent.agent_id.clone(),
            source_evidence_ids: vec![evidence.evidence_id.clone()],
        })
        .unwrap();
    kernel
        .commit_memory_write(CommitMemoryWriteInput {
            memory_id: active_memory.memory_id.clone(),
            approved_by: "agent-os-thread-test".to_string(),
        })
        .unwrap();
    let proposed_memory = kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"decision": "do not project proposed memory"}),
            created_by_agent_id: agent.agent_id.clone(),
            source_evidence_ids: vec![evidence.evidence_id.clone()],
        })
        .unwrap();
    let invalidated_memory = kernel
        .propose_memory_write(ProposeMemoryWriteInput {
            namespace: "decisions".to_string(),
            content: json!({"decision": "do not project invalidated memory"}),
            created_by_agent_id: agent.agent_id.clone(),
            source_evidence_ids: vec![evidence.evidence_id],
        })
        .unwrap();
    kernel
        .commit_memory_write(CommitMemoryWriteInput {
            memory_id: invalidated_memory.memory_id.clone(),
            approved_by: "agent-os-thread-test".to_string(),
        })
        .unwrap();
    kernel
        .invalidate_memory(&invalidated_memory.memory_id)
        .unwrap();

    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id.clone(),
        AssertsActiveMemoryProjection {
            calls: 0,
            active_memory_id: active_memory.memory_id,
            proposed_memory_id: proposed_memory.memory_id,
            invalidated_memory_id: invalidated_memory.memory_id,
        },
    );

    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_resumes_with_persisted_failed_tool_results() {
    struct RecoversFromHydratedFailure;

    impl ModelClient for RecoversFromHydratedFailure {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let replace_results = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "apply_patch")
                .collect::<Vec<_>>();
            match replace_results.as_slice() {
                [failed] => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "apply_patch",
                            json!({
                                "workspace_root": request.workspace_root.to_string_lossy(),
                                "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-state = \"old\"\n+state = \"new\"\n*** End Patch\n"
                            }),
                            4,
                            Some("resumed edit was applied after persisted failure".to_string()),
                        ),
                    )))
                }
                [failed, completed] => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    assert_eq!(completed.status, ToolCallStatus::Completed);
                    Ok(ModelTurnResponse::single(ModelAction::Final {
                        submission: FinalSubmission {
                            summary: "Recovered after resume.".to_string(),
                            changed_artifacts: request
                                .context
                                .artifacts
                                .iter()
                                .map(|artifact| artifact.artifact_id.clone())
                                .collect(),
                            evidence_map: vec![EvidenceMapEntry {
                                claim: "resumed edit was applied after persisted failure"
                                    .to_string(),
                                evidence_refs: completed.evidence_ids.clone(),
                            }],
                            unverified_claims: Vec::new(),
                            known_risks: Vec::new(),
                            tests_run: Vec::new(),
                            tests_not_run: Vec::new(),
                            approvals: Vec::new(),
                        },
                    }))
                }
                _ => Err(AgentOsError::Validation(format!(
                    "expected persisted failed apply_patch result, found {}",
                    replace_results.len()
                ))),
            }
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-resume-failure-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(workspace.join("src/lib.rs"), "state = \"old\"\n").unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-resume-failure-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Resume after failed edit".to_string(),
            description: "Resume after a failed edit attempt".to_string(),
            acceptance_criteria: vec!["failed tool result is hydrated".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Resume after failed edit".to_string(),
            description: "Resume after a failed edit attempt".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Resume after failed apply_patch call".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let first_script = DeterministicModelClient::new(vec![DeterministicStep::ToolCall(
        ToolAction::new(
            "apply_patch",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-missing\n+state = \"new\"\n*** End Patch\n"
            }),
            4,
            Some("failed edit was persisted before resume".to_string()),
        ),
    )]);
    let mut first_config = RuntimeConfig::workspace_write(&workspace);
    first_config.max_steps = 1;
    let mut first_runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), first_script);
    let first_report = first_runtime.run_to_completion(first_config).unwrap();
    assert_eq!(first_report.status, ThreadStatus::Blocked);
    assert!(!first_report.final_submitted);
    assert!(kernel
        .state_snapshot()
        .unwrap()
        .tool_invocations
        .values()
        .any(|invocation| invocation.status == ToolCallStatus::Failed));
    let state = kernel.state_snapshot().unwrap();
    let resource_lease_ids = state
        .resource_leases
        .values()
        .filter(|lease| lease.status == ResourceLeaseStatus::Granted)
        .map(|lease| lease.resource_lease_id.clone())
        .collect::<Vec<_>>();
    let environment_lease_ids = state
        .environment_leases
        .values()
        .filter(|lease| lease.status == EnvironmentLeaseStatus::Active)
        .map(|lease| lease.environment_lease_id.clone())
        .collect::<Vec<_>>();
    drop(state);
    for lease_id in resource_lease_ids {
        kernel.release_resource_lease(&lease_id).unwrap();
    }
    for lease_id in environment_lease_ids {
        kernel.release_environment_lease(&lease_id).unwrap();
    }

    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Interrupted,
            Some("simulated process restart after failed tool".to_string()),
        )
        .unwrap();
    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Ready,
            Some("resume after failed tool".to_string()),
        )
        .unwrap();

    let mut second_runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id.clone(),
        RecoversFromHydratedFailure,
    );
    let report = second_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.tool_results.len(), 2);
    assert_eq!(report.tool_results[0].status, ToolCallStatus::Failed);
    assert_eq!(report.tool_results[1].status, ToolCallStatus::Completed);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "state = \"new\"\n"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_resumes_with_persisted_tool_results_and_artifacts() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-resume-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-resume-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Resume write".to_string(),
            description: "Write file before restart, final after restart".to_string(),
            acceptance_criteria: vec!["final can use persisted evidence".to_string()],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Write file".to_string(),
            description: "Write file".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Write result.md".to_string(),
            success_criteria: vec!["result.md exists".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let first_script = DeterministicModelClient::new(vec![DeterministicStep::ToolCall(
        ToolAction::new(
            "apply_patch",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "patch": "*** Begin Patch\n*** Add File: result.md\n+persisted before restart\n*** End Patch\n"
            }),
            4,
            Some("result file was written before restart".to_string()),
        ),
    )]);
    let mut first_runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), first_script);
    let err = first_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
    assert!(workspace.join("result.md").exists());
    assert_eq!(kernel.state_snapshot().unwrap().artifacts.len(), 1);

    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Interrupted,
            Some("simulated process restart".to_string()),
        )
        .unwrap();
    kernel
        .transition_thread(
            &agent.thread_id,
            ThreadStatus::Ready,
            Some("resume after restart".to_string()),
        )
        .unwrap();

    let second_script = DeterministicModelClient::new(vec![DeterministicStep::Final {
        summary: "Resumed and submitted final.".to_string(),
        known_risks: Vec::new(),
        tests_run: Vec::new(),
        tests_not_run: Vec::new(),
    }]);
    let mut second_runtime =
        ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), second_script);
    let report = second_runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.tool_results.len(), 1);
    assert_eq!(report.artifacts.len(), 1);
    let state = kernel.state_snapshot().unwrap();
    assert_eq!(state.final_submissions.len(), 1);
    let final_submission = state.final_submissions.get(&task.task_id).unwrap();
    assert_eq!(final_submission.changed_artifacts.len(), 1);
    assert_eq!(final_submission.evidence_map.len(), 1);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn context_budget_prunes_only_old_non_evidence_tool_results() {
    let mut context = ModelContextProjection {
        tool_results: (0..7)
            .map(|index| ToolExecutionRecord {
                call_id: format!("call_{index}"),
                tool_name: "read_file".to_string(),
                status: ToolCallStatus::Completed,
                input: Some(json!({"path": format!("large-{index}.txt")})),
                output: Some(json!({"content": "x".repeat(12_000)})),
                evidence_ids: if index == 1 {
                    vec!["ev_keep".to_string()]
                } else {
                    Vec::new()
                },
                evidence_claim: None,
            })
            .collect(),
        ..ModelContextProjection::default()
    };
    let limit = ModelLimit {
        context: 8_000,
        input: Some(4_000),
        output: 1_000,
    };

    let report = super::context_projection::prune_context_for_model_limit(&mut context, &limit);

    assert!(report.pruned());
    assert!(report.pruned_refs.iter().any(|r| r == "tool_result:call_0"));
    assert!(!report.pruned_refs.iter().any(|r| r == "tool_result:call_1"));
    assert!(context.tool_results[0]
        .output
        .as_ref()
        .and_then(|output| output.get("projection_pruned"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false));
    assert!(context.tool_results[1]
        .output
        .as_ref()
        .and_then(|output| output.get("content"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.len() == 12_000));
    assert!(context.tool_results[6]
        .output
        .as_ref()
        .and_then(|output| output.get("content"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.len() == 12_000));
}

#[test]
fn tool_result_projection_compacts_old_evidence_and_preserves_recent_model_outcomes() {
    let mut records = (0..10)
        .map(|index| ToolExecutionRecord {
            call_id: format!("call_{index}"),
            tool_name: "read_file".to_string(),
            status: ToolCallStatus::Completed,
            input: Some(json!({"path": format!("file-{index}.txt")})),
            output: Some(json!({"content": "x".repeat(8_000)})),
            evidence_ids: Vec::new(),
            evidence_claim: None,
        })
        .collect::<Vec<_>>();
    records[0].evidence_ids = vec!["ev_old".to_string()];
    records[0].evidence_claim = Some("old evidence must remain visible".to_string());
    records[7].status = ToolCallStatus::Failed;
    records[7].output = Some(json!({
        "status": "failed",
        "message": "recent failure should remain complete for the next turn"
    }));
    records[8].status = ToolCallStatus::Denied;
    records[8].output = Some(json!({
        "status": "denied",
        "message": "recent denial should remain complete for the next turn"
    }));
    records[9].status = ToolCallStatus::Running;
    records[9].output = Some(json!({
        "status": "running",
        "process_id": "proc_recent"
    }));

    let projected = super::context_projection::project_tool_results(&records);

    assert_eq!(projected.len(), 9);
    assert!(projected.iter().all(|record| record.call_id != "call_1"));
    let old_evidence = projected
        .iter()
        .find(|record| record.call_id == "call_0")
        .unwrap();
    assert_eq!(old_evidence.evidence_ids, vec!["ev_old"]);
    assert_eq!(
        old_evidence
            .output
            .as_ref()
            .and_then(|output| output.get("projection_truncated"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(old_evidence
        .output
        .as_ref()
        .and_then(|output| output.get("content"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.contains("truncated for projection")));

    let failed = projected
        .iter()
        .find(|record| record.call_id == "call_7")
        .unwrap();
    assert_eq!(failed.status, ToolCallStatus::Failed);
    assert_eq!(
        failed.output.as_ref().unwrap()["message"],
        "recent failure should remain complete for the next turn"
    );
    let denied = projected
        .iter()
        .find(|record| record.call_id == "call_8")
        .unwrap();
    assert_eq!(denied.status, ToolCallStatus::Denied);
    assert_eq!(
        denied.output.as_ref().unwrap()["message"],
        "recent denial should remain complete for the next turn"
    );
    let running = projected
        .iter()
        .find(|record| record.call_id == "call_9")
        .unwrap();
    assert_eq!(running.status, ToolCallStatus::Running);
    assert_eq!(
        running.output.as_ref().unwrap()["process_id"],
        "proc_recent"
    );
}

#[test]
fn runtime_retries_model_call_from_provider_profile_policy() {
    struct FailsOnceThenFinishes {
        calls: u32,
    }

    impl ModelClient for FailsOnceThenFinishes {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(AgentOsError::Validation(
                    "provider API error kind=transient retryable=true".to_string(),
                ));
            }
            if request.context.tool_results.is_empty() {
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "read_file",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "path": "README.md"
                        }),
                        1,
                        Some("README was inspected after provider retry".to_string()),
                    ),
                )));
            }
            let evidence_refs = request.context.tool_results[0].evidence_ids.clone();
            Ok(ModelTurnResponse::single(ModelAction::Final {
                submission: FinalSubmission {
                    summary: "Completed after retry.".to_string(),
                    changed_artifacts: Vec::new(),
                    evidence_map: vec![EvidenceMapEntry {
                        claim: "README was inspected after provider retry".to_string(),
                        evidence_refs,
                    }],
                    unverified_claims: Vec::new(),
                    known_risks: Vec::new(),
                    tests_run: Vec::new(),
                    tests_not_run: Vec::new(),
                    approvals: Vec::new(),
                },
            }))
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-retry-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("README.md"), "retry me\n").unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "runtime-retry-test".to_string(),
            created_by: "agent-os-thread-test".to_string(),
            title: "Retry".to_string(),
            description: "Retry provider call".to_string(),
            acceptance_criteria: vec!["retry is recorded".to_string()],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Retry".to_string(),
            description: "Retry".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Read README after retry".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        FailsOnceThenFinishes { calls: 0 },
    );
    let report = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(kernel.events().unwrap().iter().any(|event| {
        event.event_type == "ProviderStreamEventRecorded"
            && event.payload["stream_events"]
                .as_array()
                .is_some_and(|events| {
                    events
                        .iter()
                        .any(|stream_event| stream_event["event_type"] == json!("ProviderRetry"))
                })
    }));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn runtime_does_not_retry_non_retryable_provider_failure() {
    struct FailsWithInvalidRequest {
        calls: Arc<std::sync::atomic::AtomicU32>,
    }

    impl ModelClient for FailsWithInvalidRequest {
        fn next(&mut self, _request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(AgentOsError::Validation(
                "provider API error kind=invalid_request retryable=false".to_string(),
            ))
        }
    }

    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-no-retry-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let agent = spawn_runtime_agent(&kernel, &workspace, "Fail once without retry");
    let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut runtime = ThreadRuntime::new(
        kernel.clone(),
        agent.thread_id,
        FailsWithInvalidRequest {
            calls: calls.clone(),
        },
    );

    let error = runtime
        .run_to_completion(RuntimeConfig::workspace_write(&workspace))
        .unwrap_err();

    assert!(error.to_string().contains("retryable=false"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!kernel.events().unwrap().iter().any(|event| {
        event.event_type == "ProviderStreamEventRecorded"
            && event.payload["stream_events"]
                .as_array()
                .is_some_and(|events| {
                    events
                        .iter()
                        .any(|stream_event| stream_event["event_type"] == json!("ProviderRetry"))
                })
    }));
    let _ = fs::remove_dir_all(workspace);
}
