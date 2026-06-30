use super::*;
use crate::ModelTurnResponse;
use agent_os_kernel::{RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
use std::{collections::VecDeque, env, fs};

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
fn deterministic_runtime_finishes_code_task_through_tool_loop() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-thread-runtime-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
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
            role_profile_id: "role_worker".to_string(),
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
            "replace_text",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs",
                "old": "1",
                "new": "2"
            }),
            4,
            Some("exact repository edit was applied".to_string()),
        )),
        DeterministicStep::ToolCall(ToolAction::new(
            "run_command",
            json!({
                "program": current_exe.to_string_lossy(),
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
    assert_eq!(report.tool_results.len(), 3);
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
            role_profile_id: "role_worker".to_string(),
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
            role_profile_id: "role_worker".to_string(),
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
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Make one edit but do not submit final".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let script = DeterministicModelClient::new(vec![DeterministicStep::ToolCall(ToolAction::new(
        "replace_text",
        json!({
            "workspace_root": workspace.to_string_lossy(),
            "path": "result.txt",
            "old": "before",
            "new": "after"
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
                return Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "read_file",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "path": "large.txt"
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
    fs::write(workspace.join("large.txt"), "x".repeat(12_000)).unwrap();
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
            role_profile_id: "role_worker".to_string(),
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
                .filter(|result| result.tool_name == "replace_text")
                .collect::<Vec<_>>();
            match replace_results.as_slice() {
                [] => Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "replace_text",
                        json!({
                            "workspace_root": request.workspace_root.to_string_lossy(),
                            "path": "src/lib.rs",
                            "old": "missing",
                            "new": "value = \"new\""
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
                    assert!(error.contains("replace_text expected exactly one match"));
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "replace_text",
                            json!({
                                "workspace_root": request.workspace_root.to_string_lossy(),
                                "path": "src/lib.rs",
                                "old": "value = \"old\"",
                                "new": "value = \"new\""
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
                    "unexpected replace_text retry count".to_string(),
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
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Recover after a failed replace_text call".to_string(),
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
                            "program": "cmd",
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
                                "program": "agent-os-definitely-missing-executable",
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
            role_profile_id: "role_worker".to_string(),
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
fn runtime_resumes_with_persisted_failed_tool_results() {
    struct RecoversFromHydratedFailure;

    impl ModelClient for RecoversFromHydratedFailure {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let replace_results = request
                .context
                .tool_results
                .iter()
                .filter(|result| result.tool_name == "replace_text")
                .collect::<Vec<_>>();
            match replace_results.as_slice() {
                [failed] => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "replace_text",
                            json!({
                                "workspace_root": request.workspace_root.to_string_lossy(),
                                "path": "src/lib.rs",
                                "old": "state = \"old\"",
                                "new": "state = \"new\""
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
                    "expected persisted failed replace_text result, found {}",
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
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Resume after failed replace_text call".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let first_script =
        DeterministicModelClient::new(vec![DeterministicStep::ToolCall(ToolAction::new(
            "replace_text",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "src/lib.rs",
                "old": "missing",
                "new": "state = \"new\""
            }),
            4,
            Some("failed edit was persisted before resume".to_string()),
        ))]);
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
            role_profile_id: "role_worker".to_string(),
            owner: "agent-os-thread-test".to_string(),
            goal: "Write result.md".to_string(),
            success_criteria: vec!["result.md exists".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let first_script =
        DeterministicModelClient::new(vec![DeterministicStep::ToolCall(ToolAction::new(
            "write_file",
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "result.md",
                "content": "persisted before restart\n"
            }),
            4,
            Some("result file was written before restart".to_string()),
        ))]);
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
fn runtime_retries_model_call_from_provider_profile_policy() {
    struct FailsOnceThenFinishes {
        calls: u32,
    }

    impl ModelClient for FailsOnceThenFinishes {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(AgentOsError::Validation(
                    "temporary provider failure".to_string(),
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
            role_profile_id: "role_worker".to_string(),
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
