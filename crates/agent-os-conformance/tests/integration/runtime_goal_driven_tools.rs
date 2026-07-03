use crate::common::*;
use agent_os_store::LocalBlobStore;
use agent_os_thread::{
    ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, RuntimeConfig,
    RuntimeRunOverrides, ThreadRuntime, ToolAction,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[test]
fn goal_driven_runtime_integration_covers_tools_and_agent_control_actions() {
    let fx = runtime_fixture("agent-os-runtime-integration-all-tools");
    let workspace_root = fx.workspace.to_string_lossy().to_string();
    fs::write(fx.workspace.join("read.txt"), "read me\n").unwrap();
    fs::write(
        fx.workspace.join("shot.png"),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    )
    .unwrap();
    fs::write(fx.workspace.join("edit.txt"), "alpha old beta\n").unwrap();
    fs::write(fx.workspace.join("delete.txt"), "remove me\n").unwrap();

    let script = DeterministicModelClient::new(vec![
        tool(
            "read_file",
            json!({"workspace_root": workspace_root.clone(), "path": "read.txt"}),
            1,
        ),
        tool(
            "glob_files",
            json!({
                "workspace_root": workspace_root.clone(),
                "pattern": "read.txt",
                "limit": 10
            }),
            1,
        ),
        tool(
            "grep_files",
            json!({
                "workspace_root": workspace_root.clone(),
                "pattern": "read me",
                "path": "read.txt",
                "limit": 10
            }),
            1,
        ),
        tool(
            "read_image",
            json!({"workspace_root": workspace_root.clone(), "path": "shot.png"}),
            1,
        ),
        tool(
            "apply_patch",
            json!({
                "workspace_root": workspace_root.clone(),
                "patch": "*** Begin Patch\n*** Add File: created.txt\n+created through goal-driven integration\n*** End Patch\n"
            }),
            4,
        ),
        tool(
            "apply_patch",
            json!({
                "workspace_root": workspace_root.clone(),
                "patch": "*** Begin Patch\n*** Update File: edit.txt\n@@\n-alpha old beta\n+alpha new beta\n*** End Patch\n"
            }),
            4,
        ),
        tool(
            "apply_patch",
            json!({
                "workspace_root": workspace_root.clone(),
                "patch": "*** Begin Patch\n*** Delete File: delete.txt\n*** End Patch\n"
            }),
            4,
        ),
        tool(
            "run_command",
            json!({
                "mode": "exec",
                "command": std::env::current_exe().unwrap().to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace_root.clone()
            }),
            4,
        ),
        tool(
            "set_goal",
            json!({"goal": "complete every model-visible tool in a runtime goal"}),
            2,
        ),
        tool(
            "update_checklist",
            json!({"items": [
                {"text": "exercise every model-visible tool", "status": "completed"}
            ]}),
            2,
        ),
        tool(
            "record_evidence",
            json!({
                "evidence_type": "external_reference",
                "claim": "goal-driven integration recorded explicit evidence",
                "blob_ref": "blob://goal-integration",
                "content_hash": "goal-integration-hash"
            }),
            2,
        ),
        tool(
            "report_supervisor",
            json!({"message": "goal-driven integration exercised status reporting"}),
            1,
        ),
        tool(
            "post_blackboard",
            json!({
                "channel_id": "test-results",
                "scope": "goal",
                "section": "test_result",
                "content": {"result": "goal-driven all-tool integration is running"}
            }),
            2,
        ),
        tool(
            "ask_human",
            json!({
                "question": "Confirm goal-driven integration human route wiring?",
                "context": {"test": "goal_driven_runtime_integration"}
            }),
            2,
        ),
        agent_control(
            "start",
            json!({
                "payload": {
                    "goal": "inspect child task from goal-driven integration",
                    "success_criteria": ["child was spawned"]
                }
            }),
            4,
        ),
        agent_control(
            "set_hook",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {
                    "prompt": "Report one concise integration status sentence.",
                    "interval_seconds": 30,
                    "max_response_chars": 120
                }
            }),
            4,
        ),
        agent_control(
            "send",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {"message": "continue the integration target task"}
            }),
            4,
        ),
        agent_control(
            "set_timeout",
            json!({
                "thread_id": fx.resume_thread_id,
                "payload": {"timeout_seconds": 90}
            }),
            4,
        ),
        agent_control(
            "delete_session",
            json!({"thread_id": fx.resume_thread_id}),
            6,
        ),
        agent_control("status", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("output", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("export_trace", json!({"thread_id": fx.resume_thread_id}), 1),
        agent_control("resume", json!({"thread_id": fx.resume_thread_id}), 4),
        agent_control("stop", json!({"thread_id": fx.stop_thread_id}), 4),
        agent_control("kill", json!({"thread_id": fx.kill_thread_id}), 6),
        agent_control("purge_state", json!({"thread_id": fx.purge_thread_id}), 6),
        tool(
            "accomplish_goal",
            json!({"summary": "Goal-driven runtime local goal accomplished."}),
            2,
        ),
        DeterministicStep::Final {
            summary: "Goal-driven runtime covered all model-visible tools.".to_string(),
            known_risks: Vec::new(),
            tests_run: vec![
                "goal_driven_runtime_integration_covers_tools_and_agent_control_actions"
                    .to_string(),
            ],
            tests_not_run: Vec::new(),
        },
    ]);

    let mut runtime =
        ThreadRuntime::new(fx.kernel.clone(), fx.supervisor_thread_id.clone(), script);
    let mut config = RuntimeConfig::workspace_write(&fx.workspace);
    config.max_steps = 34;
    config.tool_risk_ceiling = 6;
    config.auto_commit_patch_artifacts = false;
    let overrides = RuntimeRunOverrides {
        sandbox_profile_id: Some("sbox_workspace_write".to_string()),
        tool_approval_id: Some(fx.tool_approval_id.clone()),
    };
    let report = match runtime.run_to_completion_with_overrides(config, overrides) {
        Ok(report) => report,
        Err(error) => {
            let invocations = fx.kernel.state_snapshot().unwrap().tool_invocations;
            panic!("runtime failed: {error:?}; tool_invocations={invocations:#?}");
        }
    };

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert_eq!(report.tool_results.len(), 27);
    assert_eq!(
        fs::read_to_string(fx.workspace.join("created.txt")).unwrap(),
        "created through goal-driven integration\n"
    );
    assert_eq!(
        fs::read_to_string(fx.workspace.join("edit.txt")).unwrap(),
        "alpha new beta\n"
    );
    assert!(!fx.workspace.join("delete.txt").exists());

    let observed_tools = report
        .tool_results
        .iter()
        .map(|record| record.tool_name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_tools,
        BTreeSet::from([
            "agent_control",
            "apply_patch",
            "ask_human",
            "glob_files",
            "grep_files",
            "post_blackboard",
            "read_file",
            "read_image",
            "record_evidence",
            "report_supervisor",
            "run_command",
            "set_goal",
            "accomplish_goal",
            "update_checklist",
        ])
    );

    let observed_agent_actions = report
        .tool_results
        .iter()
        .filter(|record| record.tool_name == "agent_control")
        .filter_map(|record| record.input.as_ref())
        .filter_map(|input| input.get("action"))
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_agent_actions,
        BTreeSet::from([
            "export_trace",
            "delete_session",
            "kill",
            "output",
            "purge_state",
            "resume",
            "send",
            "set_hook",
            "set_timeout",
            "start",
            "status",
            "stop",
        ])
    );

    let state = fx.kernel.state_snapshot().unwrap();
    let final_submission = state.final_submissions.get(&fx.task_id).unwrap();
    assert_eq!(
        final_submission.summary,
        "Goal-driven runtime covered all model-visible tools."
    );
    assert!(final_submission.evidence_map.len() >= 5);
    assert_eq!(
        state.threads.get(&fx.resume_thread_id).unwrap().status,
        ThreadStatus::Ready
    );
    assert_eq!(
        state
            .threads
            .get(&fx.resume_thread_id)
            .unwrap()
            .budgets
            .wall_time_budget_ms,
        Some(90_000)
    );
    assert_eq!(
        state.threads.get(&fx.stop_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert_eq!(
        state.threads.get(&fx.kill_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert_eq!(
        state.threads.get(&fx.purge_thread_id).unwrap().status,
        ThreadStatus::Terminated
    );
    assert!(state.agent_control_commands.values().any(|command| {
        command.action == AgentControlAction::DeleteSession
            && command.status == AgentControlCommandStatus::Applied
    }));
    assert!(state.agent_control_commands.values().any(|command| {
        command.action == AgentControlAction::PurgeState
            && command.status == AgentControlCommandStatus::Applied
    }));

    write_audit_log(
        "goal-driven-all-tools-integration.jsonl",
        &[
            json!({"type": "goal", "task_id": fx.task_id, "workspace": workspace_root}),
            json!({"type": "runtime_report", "report": report}),
            json!({"type": "final_submission", "submission": final_submission}),
            json!({"type": "agent_control_actions", "actions": observed_agent_actions}),
        ],
    );
    let _ = fs::remove_dir_all(fx.workspace);
}

#[test]
fn goal_driven_runtime_integration_rejects_understated_privileged_agent_control_risk() {
    struct UnderstatedRiskRecoveryModel {
        action: String,
        target_thread_id: String,
        risk_level: u8,
        workspace_root: String,
        current_exe: String,
    }

    impl ModelClient for UnderstatedRiskRecoveryModel {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            let agent_control_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "agent_control");
            let read_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "read_file");
            let write_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "apply_patch");
            let command_result = request
                .context
                .tool_results
                .iter()
                .find(|result| result.tool_name == "run_command");
            match (
                agent_control_result,
                read_result,
                write_result,
                command_result,
            ) {
                (None, _, _, _) => Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                    ToolAction::new(
                        "agent_control",
                        json!({
                            "action": self.action.clone(),
                            "thread_id": self.target_thread_id.clone()
                        }),
                        self.risk_level,
                        Some(
                            "understated privileged agent_control action was attempted".to_string(),
                        ),
                    ),
                ))),
                (Some(failed), None, _, _) => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "read_file",
                            json!({
                                "workspace_root": self.workspace_root.clone(),
                                "path": "risk_seed.txt"
                            }),
                            1,
                            Some(
                                "understated privileged action failure seed was inspected"
                                    .to_string(),
                            ),
                        ),
                    )))
                }
                (Some(failed), Some(_), None, _) => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "apply_patch",
                            json!({
                                "workspace_root": self.workspace_root.clone(),
                                "patch": "*** Begin Patch\n*** Add File: risk_diff.txt\n+understated privileged action failed as a tool result\n*** End Patch\n"
                            }),
                            4,
                            Some("understated privileged action failure was written".to_string()),
                        ),
                    )))
                }
                (Some(failed), Some(_), Some(_), None) => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    Ok(ModelTurnResponse::single(ModelAction::ToolCall(
                        ToolAction::new(
                            "run_command",
                            json!({
                                "mode": "exec",
                                "command": self.current_exe.clone(),
                                "args": ["--help"],
                                "cwd": self.workspace_root.clone()
                            }),
                            4,
                            Some("understated privileged action recovery command ran".to_string()),
                        ),
                    )))
                }
                (Some(failed), Some(_), Some(_), Some(_)) => {
                    assert_eq!(failed.status, ToolCallStatus::Failed);
                    let evidence_map = request
                        .context
                        .tool_results
                        .iter()
                        .filter(|result| !result.evidence_ids.is_empty())
                        .map(|result| {
                            let claim = result.evidence_claim.clone().ok_or_else(|| {
                                AgentOsError::Validation(format!(
                                    "tool {} omitted evidence claim",
                                    result.tool_name
                                ))
                            })?;
                            Ok(EvidenceMapEntry {
                                claim,
                                evidence_refs: result.evidence_ids.clone(),
                            })
                        })
                        .collect::<AgentOsResult<Vec<_>>>()?;
                    Ok(ModelTurnResponse::single(ModelAction::Final {
                        submission: FinalSubmission {
                            summary:
                                "Understated privileged agent_control action failed as a tool result."
                                    .to_string(),
                            changed_artifacts: Vec::new(),
                            evidence_map,
                            unverified_claims: Vec::new(),
                            known_risks: Vec::new(),
                            tests_run: vec![
                                "goal_driven_runtime_integration_rejects_understated_privileged_agent_control_risk"
                                    .to_string(),
                            ],
                            tests_not_run: Vec::new(),
                            approvals: Vec::new(),
                        },
                    }))
                }
            }
        }
    }

    for case in [
        RejectionCase {
            action: "kill",
            risk_level: 4,
        },
        RejectionCase {
            action: "delete_session",
            risk_level: 4,
        },
        RejectionCase {
            action: "purge_state",
            risk_level: 4,
        },
    ] {
        let fx = runtime_fixture(&format!(
            "agent-os-runtime-integration-reject-{}",
            case.action
        ));
        fs::write(fx.workspace.join("risk_seed.txt"), "risk seed\n").unwrap();
        let target_thread_id = match case.action {
            "kill" => fx.kill_thread_id.clone(),
            "delete_session" => fx.resume_thread_id.clone(),
            "purge_state" => fx.purge_thread_id.clone(),
            _ => unreachable!("unexpected rejection action {}", case.action),
        };
        let script = UnderstatedRiskRecoveryModel {
            action: case.action.to_string(),
            target_thread_id,
            risk_level: case.risk_level,
            workspace_root: fx.workspace.to_string_lossy().to_string(),
            current_exe: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        };
        let mut runtime =
            ThreadRuntime::new(fx.kernel.clone(), fx.supervisor_thread_id.clone(), script);
        let mut config = RuntimeConfig::workspace_write(&fx.workspace);
        config.max_steps = 5;
        config.tool_risk_ceiling = 6;
        config.auto_commit_patch_artifacts = false;
        let overrides = RuntimeRunOverrides {
            sandbox_profile_id: Some("sbox_workspace_write".to_string()),
            tool_approval_id: Some(fx.tool_approval_id.clone()),
        };
        let report = match runtime.run_to_completion_with_overrides(config, overrides) {
            Ok(report) => report,
            Err(error) => {
                let state = fx.kernel.state_snapshot().unwrap();
                panic!(
                    "runtime failed for {}: {error:?}; tool_invocations={:#?}; evidence={:#?}",
                    case.action, state.tool_invocations, state.evidence
                );
            }
        };
        assert_eq!(report.status, ThreadStatus::Completed);
        assert!(report.final_submitted);

        let state = fx.kernel.state_snapshot().unwrap();
        assert!(state.tool_invocations.values().any(|invocation| {
            invocation.tool_name == "agent_control"
                && invocation.status == ToolCallStatus::Failed
                && invocation.input.get("action").and_then(Value::as_str) == Some(case.action)
        }));
        let expected_action = match case.action {
            "kill" => AgentControlAction::Kill,
            "delete_session" => AgentControlAction::DeleteSession,
            "purge_state" => AgentControlAction::PurgeState,
            _ => unreachable!("unexpected rejection action {}", case.action),
        };
        assert!(state.agent_control_commands.values().any(|command| {
            command.action == expected_action
                && command.status == AgentControlCommandStatus::Rejected
        }));
        write_audit_log(
            &format!(
                "goal-driven-agent-control-{}-rejection-integration.jsonl",
                case.action
            ),
            &[
                json!({"type": "rejection_case", "action": case.action, "risk_level": case.risk_level}),
                json!({"type": "runtime_report", "report": report}),
                json!({"type": "tool_invocations", "invocations": state.tool_invocations}),
                json!({"type": "agent_control_commands", "commands": state.agent_control_commands}),
            ],
        );
        let _ = fs::remove_dir_all(fx.workspace);
    }
}

#[test]
fn goal_driven_runtime_integration_covers_control_plane_optional_parameters() {
    struct ControlPlaneOptionalModel {
        task_id: String,
        step: u8,
    }

    impl ModelClient for ControlPlaneOptionalModel {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            match self.step {
                0 => {
                    self.step += 1;
                    Ok(ModelTurnResponse {
                        actions: vec![ModelAction::ToolCall(ToolAction::new(
                            "record_evidence",
                            json!({
                                "evidence_type": "test_result",
                                "claim": "control plane optional parameter evidence",
                                "task_id": self.task_id,
                                "inline_content": "OPTIONAL_PARAMETER_EVIDENCE",
                                "metadata": {
                                    "marker": "control-plane-optional",
                                    "attempt": 1
                                }
                            }),
                            2,
                            Some("optional parameter evidence was recorded".to_string()),
                        ))],
                        usage: ProviderUsage {
                            input_tokens: 1,
                            output_tokens: 1,
                            cost: 0.0,
                        },
                    })
                }
                1 => {
                    self.step += 1;
                    let evidence_id = request
                        .context
                        .tool_results
                        .iter()
                        .find(|result| result.tool_name == "record_evidence")
                        .and_then(|result| result.output.as_ref())
                        .and_then(|output| output.get("evidence_id"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            AgentOsError::Validation(format!(
                                "record_evidence result did not expose evidence_id: {:?}",
                                request.context.tool_results
                            ))
                        })?
                        .to_string();
                    Ok(ModelTurnResponse {
                        actions: vec![
                            ModelAction::ToolCall(ToolAction::new(
                                "update_checklist",
                                json!({
                                    "task_id": self.task_id,
                                    "items": [
                                        {"text": "pending optional branch", "status": "pending"},
                                        {"text": "in progress optional branch", "status": "in_progress"},
                                        {"text": "completed optional branch", "status": "completed"},
                                        {"text": "blocked optional branch", "status": "blocked"}
                                    ]
                                }),
                                2,
                                Some("all checklist item statuses were set".to_string()),
                            )),
                            ModelAction::ToolCall(ToolAction::new(
                                "report_supervisor",
                                json!({
                                    "message": "optional supervisor risk report",
                                    "message_type": "RiskReport",
                                    "artifact_refs": ["artifact_optional"],
                                    "evidence_refs": [evidence_id.clone()]
                                }),
                                2,
                                Some(
                                    "supervisor report optional parameters were routed".to_string(),
                                ),
                            )),
                            ModelAction::ToolCall(ToolAction::new(
                                "ask_human",
                                json!({
                                    "question": "Confirm optional control-plane parameter coverage?",
                                    "message_type": "HumanEscalation",
                                    "context": {
                                        "marker": "optional-human-context"
                                    },
                                    "artifact_refs": ["artifact_optional"],
                                    "evidence_refs": [evidence_id.clone()]
                                }),
                                2,
                                Some("human question optional parameters were routed".to_string()),
                            )),
                            ModelAction::ToolCall(ToolAction::new(
                                "post_blackboard",
                                json!({
                                    "channel_id": "facts",
                                    "scope": "task",
                                    "section": "known_fact",
                                    "content": {
                                        "fact": "optional parameters preserve evidence linkage"
                                    },
                                    "confidence": 0.77,
                                    "source_evidence_ids": [evidence_id.clone()]
                                }),
                                2,
                                Some("blackboard optional parameters were recorded".to_string()),
                            )),
                            ModelAction::ToolCall(ToolAction::new(
                                "set_goal",
                                json!({
                                    "goal": "control plane optional parameters verified",
                                    "title": "Optional parameter goal",
                                    "success_criteria": ["optional success criterion"],
                                    "failure_criteria": ["optional failure criterion"]
                                }),
                                2,
                                Some("goal optional parameters were set".to_string()),
                            )),
                            ModelAction::ToolCall(ToolAction::new(
                                "accomplish_goal",
                                json!({
                                    "summary": "Optional control-plane parameters complete.",
                                    "evidence_refs": [evidence_id.clone()],
                                    "artifact_refs": ["artifact_optional"],
                                    "known_risks": ["optional risk accepted"]
                                }),
                                2,
                                Some(
                                    "goal accomplishment optional parameters were set".to_string(),
                                ),
                            )),
                        ],
                        usage: ProviderUsage {
                            input_tokens: 1,
                            output_tokens: 1,
                            cost: 0.0,
                        },
                    })
                }
                2 => {
                    self.step += 1;
                    let evidence_map = request
                        .context
                        .tool_results
                        .iter()
                        .filter(|result| !result.evidence_ids.is_empty())
                        .map(|result| {
                            let claim = result.evidence_claim.clone().ok_or_else(|| {
                                AgentOsError::Validation(format!(
                                    "tool {} omitted evidence claim",
                                    result.tool_name
                                ))
                            })?;
                            Ok(EvidenceMapEntry {
                                claim,
                                evidence_refs: result.evidence_ids.clone(),
                            })
                        })
                        .collect::<AgentOsResult<Vec<_>>>()?;
                    Ok(ModelTurnResponse {
                        actions: vec![ModelAction::Final {
                            submission: FinalSubmission {
                                summary:
                                    "Control-plane optional parameters completed through runtime."
                                        .to_string(),
                                changed_artifacts: Vec::new(),
                                evidence_map,
                                unverified_claims: Vec::new(),
                                known_risks: Vec::new(),
                                tests_run: vec![
                                    "goal_driven_runtime_integration_covers_control_plane_optional_parameters"
                                        .to_string(),
                                ],
                                tests_not_run: Vec::new(),
                                approvals: Vec::new(),
                            },
                        }],
                        usage: ProviderUsage {
                            input_tokens: 1,
                            output_tokens: 1,
                            cost: 0.0,
                        },
                    })
                }
                _ => Err(AgentOsError::Validation(
                    "control-plane optional model was called after final".to_string(),
                )),
            }
        }
    }

    let workspace = temp_workspace("agent-os-runtime-control-plane-optional");
    fs::create_dir_all(&workspace).unwrap();
    let artifact_blobs = LocalBlobStore::new(workspace.join("artifacts")).unwrap();
    let evidence_blobs = LocalBlobStore::new(workspace.join("evidence")).unwrap();
    let kernel = Kernel::new().with_blob_stores(artifact_blobs, evidence_blobs);
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "integration".to_string(),
            created_by: "agent-os-conformance".to_string(),
            title: "Control-plane optional parameters".to_string(),
            description: "Exercise optional control-plane tool parameters".to_string(),
            acceptance_criteria: vec![
                "optional control-plane parameters persist through kernel state".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Control-plane optionals".to_string(),
            description: "Exercise optional model-visible control-plane fields".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-conformance".to_string(),
            goal: "Exercise optional control-plane parameters through the runtime.".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let approval = kernel
        .request_approval(RequestApprovalInput {
            goal_id: goal.goal_id.clone(),
            task_id: Some(task.task_id.clone()),
            requested_by_agent_id: agent.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![json!("tool:*")],
                risk_ceiling: 4,
                goal_id: goal.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
            },
            risk_level: 4,
            expires_at: None,
        })
        .unwrap();
    kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "agent-os-conformance".to_string(),
            decision_reason: Some("approve optional control-plane tool coverage".to_string()),
        })
        .unwrap();

    let script = ControlPlaneOptionalModel {
        task_id: task.task_id.clone(),
        step: 0,
    };
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), script);
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 4;
    config.tool_risk_ceiling = 4;
    config.auto_commit_patch_artifacts = false;
    let report = runtime
        .run_to_completion_with_overrides(
            config,
            RuntimeRunOverrides {
                sandbox_profile_id: Some("sbox_workspace_write".to_string()),
                tool_approval_id: Some(approval.approval_id),
            },
        )
        .unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);

    let state = kernel.state_snapshot().unwrap();
    let evidence = state
        .evidence
        .values()
        .find(|record| record.claim.as_deref() == Some("control plane optional parameter evidence"))
        .expect("record_evidence created evidence");
    assert_eq!(evidence.task_id.as_deref(), Some(task.task_id.as_str()));
    assert_eq!(evidence.evidence_type, EvidenceType::TestResult);
    assert_eq!(
        evidence.metadata.get("marker").and_then(Value::as_str),
        Some("control-plane-optional")
    );
    assert_eq!(
        evidence.metadata.get("attempt").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        evidence
            .metadata
            .get("blob_byte_len")
            .and_then(Value::as_u64),
        Some("OPTIONAL_PARAMETER_EVIDENCE".len() as u64)
    );
    let evidence_id = evidence.evidence_id.clone();

    let checklist = &state.tasks.get(&task.task_id).unwrap().checklist;
    assert_eq!(checklist.len(), 4);
    assert!(checklist
        .iter()
        .any(|item| item.status == ChecklistItemStatus::Pending));
    assert!(checklist
        .iter()
        .any(|item| item.status == ChecklistItemStatus::InProgress));
    assert!(checklist
        .iter()
        .any(|item| item.status == ChecklistItemStatus::Completed));
    assert!(checklist
        .iter()
        .any(|item| item.status == ChecklistItemStatus::Blocked));

    let supervisor_message = state
        .messages
        .values()
        .find(|message| message.message_type == "RiskReport")
        .expect("report_supervisor emitted RiskReport");
    assert_eq!(supervisor_message.route, MessageRoute::Supervisor);
    assert_eq!(supervisor_message.artifact_refs, vec!["artifact_optional"]);
    assert_eq!(supervisor_message.evidence_refs, vec![evidence_id.clone()]);

    let human_message = state
        .messages
        .values()
        .find(|message| message.message_type == "HumanEscalation")
        .expect("ask_human emitted HumanEscalation");
    assert_eq!(human_message.route, MessageRoute::Human);
    assert_eq!(
        human_message
            .payload
            .pointer("/context/marker")
            .and_then(Value::as_str),
        Some("optional-human-context")
    );
    assert_eq!(human_message.artifact_refs, vec!["artifact_optional"]);
    assert_eq!(human_message.evidence_refs, vec![evidence_id.clone()]);

    let blackboard = state
        .blackboard_entries
        .values()
        .find(|entry| entry.section == BlackboardSection::KnownFact)
        .expect("post_blackboard emitted known fact");
    assert_eq!(blackboard.confidence, Some(0.77));
    assert_eq!(blackboard.source_evidence_ids, vec![evidence_id.clone()]);
    assert_eq!(blackboard.task_id.as_deref(), Some(task.task_id.as_str()));

    let thread = state.threads.get(&agent.thread_id).unwrap();
    assert_eq!(
        thread.task.goal,
        "control plane optional parameters verified"
    );
    assert_eq!(thread.task.goal_revision, 2);
    assert_eq!(
        thread.task.success_criteria,
        vec!["optional success criterion".to_string()]
    );
    assert_eq!(
        thread.task.failure_criteria,
        vec!["optional failure criterion".to_string()]
    );

    let completion = kernel
        .events()
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == "AgentGoalAccomplished")
        .and_then(|event| serde_json::from_value::<AgentGoalCompletion>(event.payload).ok())
        .expect("goal accomplishment event");
    assert_eq!(
        completion.summary,
        "Optional control-plane parameters complete."
    );
    assert_eq!(completion.evidence_refs, vec![evidence_id]);
    assert_eq!(completion.artifact_refs, vec!["artifact_optional"]);
    assert_eq!(completion.known_risks, vec!["optional risk accepted"]);

    write_audit_log(
        "goal-driven-control-plane-optional-parameters-integration.jsonl",
        &[
            json!({"type": "runtime_report", "report": report}),
            json!({"type": "evidence_id", "evidence_id": completion.evidence_refs[0]}),
            json!({"type": "messages", "messages": state.messages}),
            json!({"type": "blackboard_entries", "entries": state.blackboard_entries}),
        ],
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn goal_driven_runtime_integration_prunes_context_pressure_and_records_compaction() {
    struct ContextPressureModel {
        workspace_root: String,
        pruned_context_ids: Vec<String>,
        current_marker: String,
        step: u8,
    }

    impl ModelClient for ContextPressureModel {
        fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
            match self.step {
                0 => {
                    self.step += 1;
                    assert_eq!(
                        request.context.context_snapshots.len(),
                        1,
                        "runtime should prune oversized older context before the provider request"
                    );
                    let retained = &request.context.context_snapshots[0];
                    assert!(
                        retained
                            .loaded_refs
                            .iter()
                            .any(|reference| reference.contains(&self.current_marker)),
                        "current scoped context marker was not retained: {retained:?}"
                    );
                    for pruned_id in &self.pruned_context_ids {
                        assert!(
                            request
                                .context
                                .context_snapshots
                                .iter()
                                .all(|snapshot| snapshot.context_id != *pruned_id),
                            "pruned context snapshot {pruned_id} leaked into model context"
                        );
                    }
                    Ok(ModelTurnResponse {
                        actions: vec![ModelAction::ToolCall(ToolAction::new(
                            "read_file",
                            json!({
                                "workspace_root": self.workspace_root,
                                "path": "seed.txt"
                            }),
                            1,
                            Some("context pressure seed was read".to_string()),
                        ))],
                        usage: ProviderUsage {
                            input_tokens: 1,
                            output_tokens: 1,
                            cost: 0.0,
                        },
                    })
                }
                1 => {
                    self.step += 1;
                    let compaction = request
                        .context
                        .context_compactions
                        .iter()
                        .find(|record| {
                            self.pruned_context_ids.iter().all(|context_id| {
                                record.superseded_refs.iter().any(|reference| {
                                    reference == &format!("context_snapshot:{context_id}")
                                })
                            })
                        })
                        .unwrap_or_else(|| {
                            panic!(
                                "generated context compaction was not projected: {:?}",
                                request.context.context_compactions
                            )
                        });
                    assert!(compaction.token_estimate > 0);
                    let evidence_map = request
                        .context
                        .tool_results
                        .iter()
                        .filter(|result| !result.evidence_ids.is_empty())
                        .map(|result| {
                            let claim = result.evidence_claim.clone().ok_or_else(|| {
                                AgentOsError::Validation(format!(
                                    "tool {} omitted evidence claim",
                                    result.tool_name
                                ))
                            })?;
                            Ok(EvidenceMapEntry {
                                claim,
                                evidence_refs: result.evidence_ids.clone(),
                            })
                        })
                        .collect::<AgentOsResult<Vec<_>>>()?;
                    Ok(ModelTurnResponse {
                        actions: vec![ModelAction::Final {
                            submission: FinalSubmission {
                                summary:
                                    "Context pressure pruning was visible to the runtime model."
                                        .to_string(),
                                changed_artifacts: Vec::new(),
                                evidence_map,
                                unverified_claims: Vec::new(),
                                known_risks: Vec::new(),
                                tests_run: vec![
                                    "goal_driven_runtime_integration_prunes_context_pressure_and_records_compaction"
                                        .to_string(),
                                ],
                                tests_not_run: Vec::new(),
                                approvals: Vec::new(),
                            },
                        }],
                        usage: ProviderUsage {
                            input_tokens: 1,
                            output_tokens: 1,
                            cost: 0.0,
                        },
                    })
                }
                _ => Err(AgentOsError::Validation(
                    "context pressure model was called after final".to_string(),
                )),
            }
        }
    }

    let workspace = temp_workspace("agent-os-runtime-context-pressure");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("seed.txt"), "context pressure seed\n").unwrap();

    let kernel = Kernel::new();
    kernel
        .register_model_alias(
            "tiny-context",
            "primary-provider",
            "primary-tiny-context-model",
            ModelCapabilities {
                streaming: true,
                tool_calling: true,
                reasoning: true,
                temperature: true,
                image_input: true,
                structured_output: true,
            },
            ModelLimit {
                context: 80_000,
                input: Some(40_000),
                output: 1_000,
            },
            "prov_default",
        )
        .unwrap();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "integration".to_string(),
            created_by: "agent-os-conformance".to_string(),
            title: "Context pressure pruning".to_string(),
            description: "Exercise runtime context-pressure pruning".to_string(),
            acceptance_criteria: vec![
                "older scoped context is pruned before model context".to_string(),
                "generated context compaction is replayable".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 1,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Prune context".to_string(),
            description: "Force scoped context pruning before a model request".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 1,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "agent-os-conformance".to_string(),
            goal: "Read the retained context pressure seed and finish with evidence.".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let oversized_ref = "x".repeat(100_000);
    let old_a = kernel
        .load_context(LoadContextInput {
            agent_id: agent.agent_id.clone(),
            task_id: task.task_id.clone(),
            loaded_refs: vec![format!("old-context-a-{oversized_ref}")],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 100_000,
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let current_marker = "current-context-pressure-marker".to_string();
    let current = kernel
        .load_context(LoadContextInput {
            agent_id: agent.agent_id.clone(),
            task_id: task.task_id.clone(),
            loaded_refs: vec![current_marker.clone()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 128,
        })
        .unwrap();

    let script = ContextPressureModel {
        workspace_root: workspace.to_string_lossy().to_string(),
        pruned_context_ids: vec![old_a.context_id.clone()],
        current_marker: current_marker.clone(),
        step: 0,
    };
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), script);
    let mut config = RuntimeConfig::workspace_write(&workspace);
    config.max_steps = 3;
    config.requested_model_alias = Some("tiny-context".to_string());
    let report = runtime.run_to_completion(config).unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert!(report.final_submitted);
    let state = kernel.state_snapshot().unwrap();
    let compaction = state
        .context_compactions
        .values()
        .find(|record| {
            record
                .superseded_refs
                .iter()
                .any(|reference| reference == &format!("context_snapshot:{}", old_a.context_id))
        })
        .unwrap_or_else(|| {
            panic!(
                "missing context pressure compaction; state compactions={:#?}",
                state.context_compactions
            )
        });
    assert_eq!(compaction.thread_id, agent.thread_id);
    assert_eq!(compaction.task_id, task.task_id);
    assert!(state.provider_stream_sessions.values().any(|session| {
        session.stream_events.iter().any(|event| {
            event.event_type == ProviderStreamEventType::ProviderWarning
                && event.payload.get("type").and_then(Value::as_str) == Some("context_pruned")
                && event
                    .payload
                    .get("pruned_refs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| {
                        refs.iter().any(|reference| {
                            reference.as_str()
                                == Some(&format!("context_snapshot:{}", old_a.context_id))
                        })
                    })
        })
    }));
    assert!(state
        .context_snapshots
        .get(&current.context_id)
        .unwrap()
        .loaded_refs
        .contains(&current_marker));

    write_audit_log(
        "goal-driven-context-pressure-pruning-integration.jsonl",
        &[
            json!({"type": "runtime_report", "report": report}),
            json!({"type": "context_compaction", "compaction": compaction}),
            json!({"type": "retained_context_id", "context_id": current.context_id}),
        ],
    );
    let _ = fs::remove_dir_all(workspace);
}

#[derive(Clone, Copy)]
struct RejectionCase {
    action: &'static str,
    risk_level: u8,
}

struct RuntimeFixture {
    kernel: Kernel,
    task_id: String,
    supervisor_thread_id: String,
    resume_thread_id: String,
    stop_thread_id: String,
    kill_thread_id: String,
    purge_thread_id: String,
    tool_approval_id: String,
    workspace: PathBuf,
}

fn runtime_fixture(prefix: &str) -> RuntimeFixture {
    let workspace = temp_workspace(prefix);
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "integration".to_string(),
            created_by: "agent-os-conformance".to_string(),
            title: "Goal-driven tool coverage".to_string(),
            description: "Exercise the full model-visible tool surface".to_string(),
            acceptance_criteria: vec![
                "all tools run through the runtime loop".to_string(),
                "final submission includes evidence".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 6,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Exercise tools".to_string(),
            description: "Exercise every model-visible tool".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![
                EvidenceType::SourceRef,
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let supervisor = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-conformance".to_string(),
            goal: "Use every model-visible tool to complete the coverage goal".to_string(),
            success_criteria: vec!["all tool actions are observable".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let approval = kernel
        .request_approval(RequestApprovalInput {
            goal_id: task.goal_id.clone(),
            task_id: Some(task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![
                    json!("tool:*"),
                    json!("instruction:*"),
                    json!("skill:*"),
                    json!("skill_file:*"),
                    json!("mcp:*"),
                ],
                risk_ceiling: 6,
                goal_id: task.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "agent-os-conformance".to_string(),
            decision_reason: Some("approve bounded integration tool coverage".to_string()),
        })
        .unwrap();
    let resume_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "resume target",
        &workspace,
    );
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    let stop_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "stop target",
        &workspace,
    );
    let kill_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "kill target",
        &workspace,
    );
    kernel
        .transition_thread(&kill_target.thread_id, ThreadStatus::Running, None)
        .unwrap();
    let purge_target = child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "purge target",
        &workspace,
    );

    RuntimeFixture {
        kernel,
        task_id: task.task_id,
        supervisor_thread_id: supervisor.thread_id,
        resume_thread_id: resume_target.thread_id,
        stop_thread_id: stop_target.thread_id,
        kill_thread_id: kill_target.thread_id,
        purge_thread_id: purge_target.thread_id,
        tool_approval_id: approval.approval_id,
        workspace,
    }
}

fn child_agent(
    kernel: &Kernel,
    task_id: &str,
    supervisor: &AgentControlBlock,
    goal: &str,
    workspace: &Path,
) -> AgentControlBlock {
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task_id.to_string(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: goal.to_string(),
            success_criteria: vec!["target action is observable".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap()
}

fn tool(tool_name: &str, input: Value, risk_level: u8) -> DeterministicStep {
    DeterministicStep::ToolCall(ToolAction::new(
        tool_name,
        input,
        risk_level,
        Some(format!("{tool_name} completed in goal-driven integration")),
    ))
}

fn agent_control(action: &str, mut input: Value, risk_level: u8) -> DeterministicStep {
    input
        .as_object_mut()
        .unwrap()
        .insert("action".to_string(), Value::String(action.to_string()));
    tool("agent_control", input, risk_level)
}

fn temp_workspace(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}

fn write_audit_log(file_name: &str, entries: &[Value]) {
    let audit_log_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(file_name);
    fs::create_dir_all(audit_log_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(&audit_log_path).unwrap();
    for entry in entries {
        writeln!(file, "{}", serde_json::to_string(entry).unwrap()).unwrap();
    }
    println!("integration_audit_log={}", audit_log_path.display());
}
