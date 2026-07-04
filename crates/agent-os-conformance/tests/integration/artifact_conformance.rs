use crate::common::*;
use agent_os_store::{BlobStore, LocalBlobStore};

#[test]
fn patch_artifact_requires_diff_evidence() {
    let fx = fixture();
    let err = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: Vec::new(),
            supersedes: None,
        })
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
}

#[test]
fn review_and_verification_are_independent() {
    let fx = fixture();
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::DiffRef))
        .unwrap();
    attach_writable_environment(&fx);
    let artifact = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id],
            supersedes: None,
        })
        .unwrap();

    let review_err = fx
        .kernel
        .request_review(RequestReviewInput {
            artifact_id: artifact.artifact_id.clone(),
            reviewer_agent_id: fx.worker.agent_id.clone(),
            focus: vec!["correctness".to_string()],
        })
        .unwrap_err();
    assert!(matches!(review_err, AgentOsError::PermissionDenied(_)));

    let verify_err = fx
        .kernel
        .submit_verification(SubmitVerificationInput {
            artifact_id: Some(artifact.artifact_id),
            final_artifact_id: None,
            verifier_agent_id: fx.worker.agent_id.clone(),
            checked_claims: Vec::new(),
            unsupported_claims: Vec::new(),
            stale_evidence_ids: Vec::new(),
            verdict: VerificationVerdict::Pass,
        })
        .unwrap_err();
    assert!(matches!(verify_err, AgentOsError::PermissionDenied(_)));
}

#[test]
fn patch_artifact_requires_writable_environment_lease() {
    let fx = fixture();
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::DiffRef))
        .unwrap();
    let err = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id.clone()],
            supersedes: None,
        })
        .unwrap_err();
    assert!(matches!(err, AgentOsError::PermissionDenied(_)));

    let env = fx
        .kernel
        .create_environment(
            BackendType::LocalProcess,
            "readonly",
            "sbox_readonly",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::ReadOnly,
        )
        .unwrap();
    let readonly_err = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id.clone()],
            supersedes: None,
        })
        .unwrap_err();
    assert!(matches!(readonly_err, AgentOsError::PermissionDenied(_)));

    attach_writable_environment(&fx);
    let artifact = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("blob://patch".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id],
            supersedes: None,
        })
        .unwrap();
    assert_eq!(artifact.artifact_type, ArtifactType::Patch);
}

#[test]
fn final_submission_requires_active_evidence_map() {
    let fx = fixture();
    let err = fx
        .kernel
        .submit_final(
            &fx.worker.agent_id,
            &fx.task.task_id,
            FinalSubmission {
                summary: "done".to_string(),
                changed_artifacts: Vec::new(),
                evidence_map: Vec::new(),
                unverified_claims: Vec::new(),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: Vec::new(),
                approvals: Vec::new(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));

    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    fx.kernel
        .submit_final(
            &fx.worker.agent_id,
            &fx.task.task_id,
            FinalSubmission {
                summary: "done".to_string(),
                changed_artifacts: Vec::new(),
                evidence_map: vec![EvidenceMapEntry {
                    claim: "source was inspected".to_string(),
                    evidence_refs: vec![evidence.evidence_id],
                }],
                unverified_claims: Vec::new(),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: vec!["none".to_string()],
                approvals: Vec::new(),
            },
        )
        .unwrap();
}

#[test]
fn submit_final_lifecycle_tool_records_final_submission() {
    let fx = fixture();
    let evidence = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::SourceRef))
        .unwrap();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "configure worker hook".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "submit final with hook cleanup".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    let supervisor_cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    fx.kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "set_hook",
                    "thread_id": worker.thread_id.clone(),
                    "payload": {
                        "interval_seconds": 30,
                        "prompt": "Report progress."
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            2,
            None,
        )
        .unwrap();
    let invocation = fx
        .kernel
        .invoke_tool(
            &worker.agent_id,
            &fx.task.task_id,
            &worker.session_id,
            cap.capability_id,
            2,
            ToolInvokeInput {
                tool_name: "submit_final".to_string(),
                input: json!({
                    "summary": "done",
                    "evidence_map": [{
                        "claim": "source was inspected",
                        "evidence_refs": [evidence.evidence_id]
                    }],
                    "tests_not_run": ["not required"]
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(invocation.status, ToolCallStatus::Completed);
    assert_eq!(
        invocation
            .output
            .as_ref()
            .and_then(|output| output.get("final_submitted"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert!(fx
        .kernel
        .state_snapshot()
        .unwrap()
        .final_submissions
        .contains_key(&fx.task.task_id));
    assert!(fx
        .kernel
        .state_snapshot()
        .unwrap()
        .agent_hooks
        .values()
        .any(
            |hook| hook.thread_id == worker.thread_id && hook.status == AgentHookStatus::Completed
        ));
}

#[test]
fn submit_final_failures_are_model_visible_without_final_side_effects() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            2,
            None,
        )
        .unwrap();
    let before = fx.kernel.state_snapshot().unwrap();
    let before_thread = before.threads.get(&fx.worker.thread_id).unwrap().clone();
    let before_final_count = before.final_submissions.len();
    let before_verification_count = before.verifications.len();
    let before_evidence_count = before.evidence.len();

    for (input, expected_stage, expected_error) in [
        (
            json!({
                "summary": "empty evidence map",
                "evidence_map": []
            }),
            "input_schema",
            "tool.input.evidence_map length must be >= 1",
        ),
        (
            json!({
                "summary": "missing evidence refs",
                "evidence_map": [{"claim": "claim without refs"}]
            }),
            "input_schema",
            "tool.input.evidence_map[0] missing required field evidence_refs",
        ),
        (
            json!({
                "summary": "tests_run must be array",
                "evidence_map": [{
                    "claim": "shape validation",
                    "evidence_refs": ["evi_shape"]
                }],
                "tests_run": "cargo test"
            }),
            "input_schema",
            "tool.input.tests_run expected array",
        ),
        (
            json!({
                "summary": "empty refs",
                "evidence_map": [{
                    "claim": "claim without refs",
                    "evidence_refs": []
                }]
            }),
            "driver",
            "claim 'claim without refs' lacks evidence refs",
        ),
        (
            json!({
                "summary": "missing evidence",
                "evidence_map": [{
                    "claim": "references missing evidence",
                    "evidence_refs": ["evi_missing"]
                }]
            }),
            "driver",
            "evidence evi_missing",
        ),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                2,
                ToolInvokeInput {
                    tool_name: "submit_final".to_string(),
                    input,
                    evidence_claim: Some("submit_final failure was model-visible".to_string()),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let output = invocation.output.as_ref().unwrap();
        assert_eq!(output["status"], "failed");
        let error = output["error"].as_str().unwrap_or_default();
        assert_eq!(
            output["stage"], expected_stage,
            "expected stage {expected_stage:?} for {expected_error:?}, got {error:?}"
        );
        assert!(
            error.contains(expected_error),
            "expected {expected_error:?}, got {error:?}"
        );

        let state = fx.kernel.state_snapshot().unwrap();
        let thread = state.threads.get(&fx.worker.thread_id).unwrap();
        assert_eq!(state.final_submissions.len(), before_final_count);
        assert_eq!(state.verifications.len(), before_verification_count);
        assert_eq!(state.evidence.len(), before_evidence_count);
        assert_eq!(thread.status, before_thread.status);
        assert_eq!(thread.invocation_id, before_thread.invocation_id);
    }
}

#[test]
fn tool_invocation_routes_through_kernel_and_attaches_evidence() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-command-tool-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "tool.invoke",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        4,
        json!({
            "tool_name": "run_command",
            "input": {
                "mode": "exec",
                "command": std::env::current_exe().unwrap().to_string_lossy(),
                "args": ["--help"],
                "cwd": workspace.to_string_lossy()
            },
            "evidence_claim": "tests were run"
        }),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    assert!(result.accepted);
    let state = fx.kernel.state_snapshot().unwrap();
    let invocation = state.tool_invocations.values().next().unwrap();
    assert_eq!(invocation.status, ToolCallStatus::Completed);
    assert_eq!(invocation.evidence_ids.len(), 1);
    let evidence = state.evidence.get(&invocation.evidence_ids[0]).unwrap();
    assert_eq!(evidence.evidence_type, EvidenceType::CommandLog);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn workspace_and_process_tools_execute_through_broker_and_attach_evidence() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-real-tools-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();

    let denied = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "apply_patch".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "patch": "*** Begin Patch\n*** Add File: result.md\n+hello\n*** End Patch\n"
                }),
                evidence_claim: Some("workspace file was created through apply_patch".to_string()),
            },
        )
        .unwrap();
    assert_eq!(denied.status, ToolCallStatus::Failed);
    let error = denied
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(error.contains("active compatible environment lease"));

    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();

    let write = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "apply_patch".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "patch": "*** Begin Patch\n*** Add File: result.md\n+hello\n*** End Patch\n"
                }),
                evidence_claim: Some("workspace file was created through apply_patch".to_string()),
            },
        )
        .unwrap();
    assert_eq!(write.status, ToolCallStatus::Completed);
    assert_eq!(write.evidence_ids.len(), 1);
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .evidence
            .get(&write.evidence_ids[0])
            .unwrap()
            .evidence_type,
        EvidenceType::DiffRef
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("result.md")).unwrap(),
        "hello\n"
    );

    let read = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "read_file".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "result.md"
                }),
                evidence_claim: Some("workspace file was inspected".to_string()),
            },
        )
        .unwrap();
    assert_eq!(read.status, ToolCallStatus::Completed);
    assert_eq!(read.evidence_ids.len(), 1);
    assert_eq!(
        read.output
            .as_ref()
            .and_then(|output| output.get("content"))
            .and_then(|value| value.as_str()),
        Some("hello\n")
    );

    let replace = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "apply_patch".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "patch": "*** Begin Patch\n*** Update File: result.md\n@@\n-hello\n+hello from replace\n*** End Patch\n"
                }),
                evidence_claim: Some("workspace file was updated through apply_patch".to_string()),
            },
        )
        .unwrap();
    assert_eq!(replace.status, ToolCallStatus::Completed);
    assert_eq!(replace.evidence_ids.len(), 1);
    assert_eq!(
        std::fs::read_to_string(workspace.join("result.md")).unwrap(),
        "hello from replace\n"
    );

    let delete = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "apply_patch".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "patch": "*** Begin Patch\n*** Delete File: result.md\n*** End Patch\n"
                }),
                evidence_claim: Some("workspace file was deleted through apply_patch".to_string()),
            },
        )
        .unwrap();
    assert_eq!(delete.status, ToolCallStatus::Completed);
    assert_eq!(delete.evidence_ids.len(), 1);
    assert!(!workspace.join("result.md").exists());

    let process = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "mode": "exec",
                    "command": std::env::current_exe().unwrap().to_string_lossy(),
                    "args": ["--help"],
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("process tool executed".to_string()),
            },
        )
        .unwrap();
    assert_eq!(process.status, ToolCallStatus::Completed);
    let process_output = process.output.as_ref().unwrap();
    let process_id = process_output["process_id"].as_str().unwrap();
    assert_eq!(process.evidence_ids.len(), 1);
    let state = fx.kernel.state_snapshot().unwrap();
    assert_eq!(
        state
            .evidence
            .get(&process.evidence_ids[0])
            .unwrap()
            .evidence_type,
        EvidenceType::CommandLog
    );
    let session = state.process_sessions.get(process_id).unwrap();
    assert_eq!(session.tool_call_id, process.call_id);
    assert_eq!(session.agent_id, fx.worker.agent_id);
    assert_eq!(session.thread_id, fx.worker.thread_id);
    assert_eq!(session.state, ProcessLifecycleState::Exited);
    assert_eq!(session.exit_code, Some(0));
    assert_eq!(session.command_mode, ProcessCommandMode::Exec);
    assert_eq!(session.stdout.cursor, session.stdout.bytes);
    assert_eq!(
        session.stdout.bytes,
        process_output["stdout_bytes"].as_u64().unwrap()
    );
    let process_chunks = state
        .process_output_chunks
        .iter()
        .filter(|chunk| chunk.process_id == process_id)
        .collect::<Vec<_>>();
    assert!(process_chunks
        .iter()
        .any(|chunk| chunk.stream == ProcessOutputStreamName::Stdout));
    assert_eq!(
        session.stdout.sequence,
        process_chunks
            .iter()
            .filter(|chunk| chunk.stream == ProcessOutputStreamName::Stdout)
            .map(|chunk| chunk.sequence)
            .max()
            .unwrap()
    );
    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state
            .process_sessions
            .get(process_id)
            .unwrap()
            .state,
        ProcessLifecycleState::Exited
    );
    assert_eq!(
        replayed_state
            .process_output_chunks
            .iter()
            .filter(|chunk| chunk.process_id == process_id)
            .count(),
        process_chunks.len()
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn read_file_paginates_with_offset_limit_metadata() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-read-page-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    std::fs::write(workspace.join("paged.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();

    let read = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "read_file".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "paged.txt",
                    "offset": 1,
                    "limit": 2
                }),
                evidence_claim: Some("workspace file page was inspected".to_string()),
            },
        )
        .unwrap();

    assert_eq!(read.status, ToolCallStatus::Completed);
    let output = read.output.as_ref().unwrap();
    assert_eq!(output["content"], json!("one\ntwo\n"));
    assert_eq!(output["offset"], json!(1));
    assert_eq!(output["limit"], json!(2));
    assert_eq!(output["total_lines"], json!(4));
    assert_eq!(output["returned_lines"], json!(2));
    assert_eq!(output["next_offset"], json!(3));
    assert_eq!(output["truncated"], json!(true));
    assert_eq!(output["omitted_lines"], json!(2));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn read_file_reports_parameter_and_driver_failures_to_model() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-read-file-failures-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    std::fs::write(workspace.join("paged.txt"), "one\ntwo\n").unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();

    for (input, expected_stage, expected_error) in [
        (
            json!({
                "workspace_root": workspace.to_string_lossy(),
            }),
            "input_schema",
            "tool.input missing required field path",
        ),
        (
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": 7,
            }),
            "input_schema",
            "tool.input.path expected string",
        ),
        (
            json!({
                "path": "paged.txt",
            }),
            "input_schema",
            "tool.input missing required field workspace_root",
        ),
        (
            json!({
                "workspace_root": 7,
                "path": "paged.txt",
            }),
            "input_schema",
            "tool.input.workspace_root expected string",
        ),
        (
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "paged.txt",
                "offset": 0,
                "limit": 1
            }),
            "input_schema",
            "tool.input.offset must be >= 1",
        ),
        (
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "paged.txt",
                "offset": 1,
                "limit": 1001
            }),
            "input_schema",
            "tool.input.limit must be <= 1000",
        ),
        (
            json!({
                "workspace_root": workspace.to_string_lossy(),
                "path": "missing.txt",
                "offset": 1,
                "limit": 1
            }),
            "driver",
            "read workspace file",
        ),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "read_file".to_string(),
                    input,
                    evidence_claim: Some("read_file failure was model-visible".to_string()),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let output = invocation.output.as_ref().unwrap();
        assert_eq!(output["status"], "failed");
        assert_eq!(output["stage"], expected_stage);
        let error = output["error"].as_str().unwrap_or_default();
        assert!(
            error.contains(expected_error),
            "expected {expected_error:?}, got {error:?}"
        );
    }

    assert!(fx.kernel.state_snapshot().unwrap().evidence.is_empty());

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn read_image_returns_base64_data_url_with_source_evidence() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-read-image-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(
        workspace.join("shot.png"),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    )
    .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();

    let read = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "read_image".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "shot.png"
                }),
                evidence_claim: Some("workspace image was inspected".to_string()),
            },
        )
        .unwrap();

    assert_eq!(read.status, ToolCallStatus::Completed);
    assert_eq!(read.evidence_ids.len(), 1);
    let output = read.output.as_ref().unwrap();
    assert_eq!(output["mime_type"], json!("image/png"));
    assert_eq!(output["encoding"], json!("base64"));
    assert_eq!(output["bytes_read"], json!(8));
    assert_eq!(
        output["data_url"],
        json!("data:image/png;base64,iVBORw0KGgo=")
    );
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .evidence
            .get(&read.evidence_ids[0])
            .unwrap()
            .evidence_type,
        EvidenceType::SourceRef
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn read_image_reports_model_visible_failures() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-read-image-failures-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    std::fs::create_dir_all(workspace.join("folder.png")).unwrap();
    std::fs::write(workspace.join("vector.svg"), "<svg></svg>\n").unwrap();
    std::fs::write(workspace.join("empty.png"), []).unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();

    for (input, expected_error) in [
        (
            json!({"workspace_root": workspace.to_string_lossy()}),
            "tool.input missing required field path",
        ),
        (
            json!({"workspace_root": workspace.to_string_lossy(), "path": 7}),
            "tool.input.path expected string",
        ),
        (
            json!({"path": "empty.png"}),
            "tool.input missing required field workspace_root",
        ),
        (
            json!({"workspace_root": 7, "path": "empty.png"}),
            "tool.input.workspace_root expected string",
        ),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "read_image".to_string(),
                    input,
                    evidence_claim: Some(
                        "read_image parameter failure was model-visible".to_string(),
                    ),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let output = invocation.output.as_ref().unwrap();
        assert_eq!(output["status"], "failed");
        assert_eq!(output["stage"], "input_schema");
        let error = output["error"].as_str().unwrap_or_default();
        assert!(error.contains(expected_error), "{error}");
    }

    for (path, expected) in [
        (
            "vector.svg",
            "read_image supports png, jpg, jpeg, gif, webp, bmp, tif, tiff, avif, and ico files",
        ),
        ("empty.png", "read_image cannot read an empty image file"),
        ("folder.png", "read_image path must point to a file"),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "read_image".to_string(),
                    input: json!({
                        "workspace_root": workspace.to_string_lossy(),
                        "path": path
                    }),
                    evidence_claim: Some(format!("read_image rejected {path}")),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let error = invocation
            .output
            .as_ref()
            .and_then(|output| output.get("error"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(error.contains(expected), "{path}: {error}");
    }

    assert!(fx.kernel.state_snapshot().unwrap().evidence.is_empty());

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn long_running_command_returns_running_and_can_be_queried_by_tool_call_id() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-background-command-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "observe child tools".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "run a slow command".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &worker.agent_id,
            &worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let command_cap = fx
        .kernel
        .grant_capability(
            &worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let supervisor_approval = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![json!("tool:*")],
                risk_ceiling: 6,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: supervisor_approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: Some("approve supervisor tool progress inspection".to_string()),
        })
        .unwrap();
    let supervisor_cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            6,
            Some(supervisor_approval.approval_id),
        )
        .unwrap();

    let command = fx
        .kernel
        .invoke_tool(
            &worker.agent_id,
            &fx.task.task_id,
            &worker.session_id,
            command_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": "Write-Output stdout-before; [Console]::Error.WriteLine('stderr-before'); Start-Sleep -Seconds 18; Write-Output stdout-after",
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("background command started".to_string()),
            },
        )
        .unwrap();

    assert_eq!(command.status, ToolCallStatus::Running);
    assert_eq!(
        command
            .output
            .as_ref()
            .and_then(|output| output.get("tool_call_id"))
            .and_then(serde_json::Value::as_str),
        Some(command.call_id.as_str())
    );
    let process_id = command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap();
    let process_session = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .process_sessions
        .get(process_id)
        .unwrap()
        .clone();
    assert_eq!(process_session.tool_call_id, command.call_id);
    assert_eq!(process_session.state, ProcessLifecycleState::Running);

    let queried = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "output",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "tool_call_id": command.call_id.clone()
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(queried.status, ToolCallStatus::Completed);
    assert_eq!(
        queried
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/tool_call_id"))
            .and_then(serde_json::Value::as_str),
        command.output.as_ref().unwrap()["tool_call_id"].as_str()
    );
    let queried_output = queried.output.as_ref().unwrap();
    assert!(queried_output
        .pointer("/output/fields/stdout/new/text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("stdout-before"));
    assert!(queried_output
        .pointer("/output/fields/stderr/new/text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("stderr-before"));
    assert!(
        queried_output
            .pointer("/output/fields/stdout/bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            > 0
    );
    let first_stdout_cursor = queried_output
        .pointer("/output/fields/stdout/next_cursor")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(
        queried_output
            .pointer("/output/fields/stdout/new/end_byte")
            .and_then(serde_json::Value::as_u64),
        Some(first_stdout_cursor)
    );

    let queried_process = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "output",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "process_id": process_id,
                        "field": "stdout"
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let first_stdout_sequence = queried_process
        .output
        .as_ref()
        .and_then(|output| output.pointer("/output/process_output/next_sequence/stdout"))
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert!(first_stdout_sequence > 0);
    let process_chunk_text = queried_process
        .output
        .as_ref()
        .and_then(|output| output.pointer("/output/process_output/chunks"))
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|chunk| chunk.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    assert!(process_chunk_text.contains("stdout-before"));

    let mut stdout_new = String::new();
    let poll_started = std::time::Instant::now();
    while poll_started.elapsed() < std::time::Duration::from_secs(10) {
        let queried_new = fx
            .kernel
            .invoke_tool(
                &supervisor.agent_id,
                &fx.task.task_id,
                &supervisor.session_id,
                supervisor_cap.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "agent_control".to_string(),
                    input: json!({
                        "action": "output",
                        "thread_id": worker.thread_id,
                        "payload": {
                            "tool_call_id": command.call_id.clone(),
                            "new": 200,
                            "cursor": {
                                "stdout": first_stdout_cursor
                            }
                        }
                    }),
                    evidence_claim: None,
                },
            )
            .unwrap();
        stdout_new = queried_new
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/fields/stdout/new/text"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if stdout_new.contains("stdout-after") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    assert!(stdout_new.contains("stdout-after"));
    assert!(!stdout_new.contains("stdout-before"));

    let queried_process_new = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "output",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "process_id": process_id,
                        "field": "stdout",
                        "after_sequence": {
                            "stdout": first_stdout_sequence
                        }
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let process_new_text = queried_process_new
        .output
        .as_ref()
        .and_then(|output| output.pointer("/output/process_output/chunks"))
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|chunk| chunk.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    assert!(process_new_text.contains("stdout-after"));
    assert!(!process_new_text.contains("stdout-before"));

    let queried_page = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id,
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "output",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "tool_call_id": command.call_id.clone(),
                        "field": "stdout",
                        "full": true,
                        "offset": 1,
                        "limit": 1
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let stdout_page = queried_page.output.as_ref().unwrap();
    assert_eq!(
        stdout_page
            .pointer("/output/fields/stdout/offset")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert!(stdout_page
        .pointer("/output/fields/stdout/content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("stdout-after"));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn process_stdin_write_is_idempotent_and_pollable_by_process_id() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-stdin-command-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let mut run_command_descriptor = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .tool_descriptors
        .get("run_command")
        .unwrap()
        .clone();
    run_command_descriptor.lifecycle.foreground_timeout_ms = 500;
    fx.kernel
        .register_tool_descriptor(run_command_descriptor)
        .unwrap();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "write child process stdin".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "run a stdin command".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &worker.agent_id,
            &worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let command_cap = fx
        .kernel
        .grant_capability(
            &worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let supervisor_approval = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![json!("tool:*")],
                risk_ceiling: 6,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: supervisor_approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: Some("approve supervisor process stdin write".to_string()),
        })
        .unwrap();
    let supervisor_cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            6,
            Some(supervisor_approval.approval_id),
        )
        .unwrap();
    let stdin_command = if cfg!(windows) {
        "$line = [Console]::In.ReadLine(); Write-Output \"stdin:$line\"; Start-Sleep -Seconds 2"
    } else {
        "IFS= read -r line; printf 'stdin:%s\n' \"$line\"; sleep 2"
    };
    let command = fx
        .kernel
        .invoke_tool(
            &worker.agent_id,
            &fx.task.task_id,
            &worker.session_id,
            command_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": stdin_command,
                    "stdin": "piped",
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("stdin command started".to_string()),
            },
        )
        .unwrap();

    assert_eq!(command.status, ToolCallStatus::Running);
    let process_id = command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string();
    let mut process_session = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .process_sessions
        .get(&process_id)
        .unwrap()
        .clone();
    let running_started = std::time::Instant::now();
    while process_session.state != ProcessLifecycleState::Running
        && running_started.elapsed() < std::time::Duration::from_secs(5)
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
        process_session = fx
            .kernel
            .state_snapshot()
            .unwrap()
            .process_sessions
            .get(&process_id)
            .unwrap()
            .clone();
    }
    assert_eq!(process_session.state, ProcessLifecycleState::Running);
    assert_eq!(process_session.stdin_mode, ProcessStdinMode::Piped);

    let send_payload = json!({
        "action": "send",
        "thread_id": worker.thread_id,
        "payload": {
            "process_id": process_id,
            "write_id": "stdin-write-1",
            "text": "codex-stdin\n"
        }
    });
    let send = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: send_payload.clone(),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(send.status, ToolCallStatus::Completed, "{:?}", send.output);
    assert_eq!(
        send.output
            .as_ref()
            .and_then(|output| output.pointer("/output/stdin_write/write_id"))
            .and_then(serde_json::Value::as_str),
        Some("stdin-write-1")
    );

    let duplicate = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: send_payload,
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        duplicate
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/stdin_write/sequence"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    let state_after_writes = fx.kernel.state_snapshot().unwrap();
    assert_eq!(
        state_after_writes
            .process_stdin_writes
            .iter()
            .filter(|write| write.process_id == process_id)
            .count(),
        1
    );

    let mut stdin_output = String::new();
    let poll_started = std::time::Instant::now();
    while poll_started.elapsed() < std::time::Duration::from_secs(8) {
        let queried = fx
            .kernel
            .invoke_tool(
                &supervisor.agent_id,
                &fx.task.task_id,
                &supervisor.session_id,
                supervisor_cap.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "agent_control".to_string(),
                    input: json!({
                        "action": "output",
                        "thread_id": worker.thread_id,
                        "payload": {
                            "process_id": process_id,
                            "field": "stdout"
                        }
                    }),
                    evidence_claim: None,
                },
            )
            .unwrap();
        stdin_output = queried
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/process_output/chunks"))
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .filter_map(|chunk| chunk.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        if stdin_output.contains("stdin:codex-stdin") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(stdin_output.contains("stdin:codex-stdin"));

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state
            .process_stdin_writes
            .iter()
            .filter(|write| write.process_id == process_id)
            .count(),
        1
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn write_stdin_tool_continues_own_process_and_polls_output() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-write-stdin-tool-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string(), "process:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let stdin_command = if cfg!(windows) {
        "$line = [Console]::In.ReadLine(); Write-Output \"direct:$line\"; Start-Sleep -Seconds 2"
    } else {
        "IFS= read -r line; printf 'direct:%s\n' \"$line\"; sleep 2"
    };
    let command_started = std::time::Instant::now();
    let command = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": stdin_command,
                    "stdin": "piped",
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("direct stdin command started".to_string()),
            },
        )
        .unwrap();
    assert!(
        command_started.elapsed() < std::time::Duration::from_secs(5),
        "run_command stdin=piped should return a running process without waiting for the default foreground timeout"
    );
    assert_eq!(command.status, ToolCallStatus::Running);
    let process_id = command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string();
    wait_process_state(&fx.kernel, &process_id, ProcessLifecycleState::Running);

    let write_input = json!({
        "process_id": process_id,
        "write_id": "direct-write-1",
        "text": "model-stdin\n"
    });
    let write = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "write_stdin".to_string(),
                input: write_input.clone(),
                evidence_claim: Some("direct stdin write completed".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        write.status,
        ToolCallStatus::Completed,
        "{:?}",
        write.output
    );
    assert_eq!(
        write
            .output
            .as_ref()
            .and_then(|output| output.pointer("/stdin_write/write_id"))
            .and_then(serde_json::Value::as_str),
        Some("direct-write-1")
    );
    let duplicate = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "write_stdin".to_string(),
                input: write_input,
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        duplicate
            .output
            .as_ref()
            .and_then(|output| output.pointer("/stdin_write/sequence"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let mut stdout = String::new();
    let poll_started = std::time::Instant::now();
    while poll_started.elapsed() < std::time::Duration::from_secs(8) {
        let poll = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                4,
                ToolInvokeInput {
                    tool_name: "write_stdin".to_string(),
                    input: json!({
                        "process_id": process_id,
                        "field": "stdout"
                    }),
                    evidence_claim: None,
                },
            )
            .unwrap();
        stdout = poll
            .output
            .as_ref()
            .and_then(|output| output.pointer("/process_output/chunks"))
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .filter_map(|chunk| chunk.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        if stdout.contains("direct:model-stdin") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(stdout.contains("direct:model-stdin"));
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .process_stdin_writes
            .iter()
            .filter(|write| write.process_id == process_id)
            .count(),
        1
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn process_tools_report_parameter_failures_through_broker() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-process-parameter-failures-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    std::fs::create_dir_all(&workspace).unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string(), "process:*".to_string()],
            4,
            None,
        )
        .unwrap();

    for (tool_name, input, expected_stage, expected_error) in [
        (
            "run_command",
            json!({
                "mode": "python",
                "command": "echo should-not-run",
                "cwd": workspace.to_string_lossy()
            }),
            "input_schema",
            "tool.input.mode does not match any enum value",
        ),
        (
            "run_command",
            json!({
                "stdin": "interactive",
                "command": "echo should-not-run",
                "cwd": workspace.to_string_lossy()
            }),
            "input_schema",
            "tool.input.stdin does not match any enum value",
        ),
        (
            "run_command",
            json!({
                "command": "echo should-not-run",
                "args": "unexpected",
                "cwd": workspace.to_string_lossy()
            }),
            "input_schema",
            "tool.input.args expected array",
        ),
        (
            "run_command",
            json!({
                "command": "echo should-not-run",
                "args": ["ok", 7],
                "cwd": workspace.to_string_lossy()
            }),
            "input_schema",
            "tool.input.args[1] expected string",
        ),
        (
            "run_command",
            json!({
                "command": "echo should-not-run",
                "cwd": workspace.to_string_lossy(),
                "env": "unexpected"
            }),
            "input_schema",
            "tool.input.env expected object",
        ),
        (
            "run_command",
            json!({
                "command": "echo should-not-run",
                "cwd": workspace.to_string_lossy(),
                "env": {"AGENT_OS_TEST": 7}
            }),
            "driver",
            "run_command env values must be strings",
        ),
        (
            "run_command",
            json!({
                "mode": "shell",
                "command": "echo should-not-run",
                "args": ["unexpected"],
                "cwd": workspace.to_string_lossy()
            }),
            "driver",
            "run_command args require exec mode",
        ),
        (
            "run_command",
            json!({
                "command": "echo should-not-run",
                "cwd": workspace.to_string_lossy(),
                "env": {"": "empty-key"}
            }),
            "driver",
            "run_command env keys must not be empty",
        ),
        (
            "write_stdin",
            json!({
                "process_id": "proc_missing",
                "write_id": "stdin-no-text"
            }),
            "driver",
            "write_stdin write_id requires text",
        ),
        (
            "write_stdin",
            json!({
                "process_id": "proc_missing",
                "text": "text without write id\n"
            }),
            "driver",
            "write_stdin text requires write_id",
        ),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                cap.capability_id.clone(),
                4,
                ToolInvokeInput {
                    tool_name: tool_name.to_string(),
                    input,
                    evidence_claim: Some(format!("{tool_name} parameter failure was reported")),
                },
            )
            .unwrap();
        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        let output = invocation.output.as_ref().unwrap();
        assert_eq!(output["status"], "failed");
        let error = output["error"].as_str().unwrap_or_default();
        assert_eq!(
            output["stage"], expected_stage,
            "expected stage {expected_stage:?} for {expected_error:?}, got {tool_name}: {error:?}"
        );
        assert!(
            error.contains(expected_error),
            "expected {expected_error:?}, got {tool_name}: {error:?}"
        );
    }

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn run_command_args_without_mode_infers_exec_through_broker() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-run-command-exec-infer-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &fx.worker.agent_id,
            &fx.worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string(), "process:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let command_path = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let invocation = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": command_path,
                    "args": ["--help"],
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("run_command inferred exec mode".to_string()),
            },
        )
        .unwrap();
    assert_eq!(invocation.status, ToolCallStatus::Completed);
    assert_eq!(invocation.evidence_ids.len(), 1);
    let output = invocation.output.as_ref().unwrap();
    assert_eq!(output["execution_mode"], "exec");
    assert_eq!(output["stdin_mode"], "closed");
    assert_eq!(
        output["executed_program"].as_str(),
        Some(command_path.as_str())
    );
    assert_eq!(output["executed_args"], json!(["--help"]));
    assert_eq!(output["exit_code"], 0);
    let process_id = output["process_id"].as_str().unwrap();
    let state = fx.kernel.state_snapshot().unwrap();
    let session = state.process_sessions.get(process_id).unwrap();
    assert_eq!(session.command_mode, ProcessCommandMode::Exec);
    assert_eq!(session.stdin_mode, ProcessStdinMode::Closed);
    assert_eq!(session.args, vec!["--help".to_string()]);

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn process_stop_and_kill_record_interrupted_and_terminated_sessions() {
    let fx = fixture();
    let workspace = std::env::temp_dir().join(format!(
        "agent-os-stop-kill-command-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let mut run_command_descriptor = fx
        .kernel
        .state_snapshot()
        .unwrap()
        .tool_descriptors
        .get("run_command")
        .unwrap()
        .clone();
    run_command_descriptor.lifecycle.foreground_timeout_ms = 500;
    fx.kernel
        .register_tool_descriptor(run_command_descriptor)
        .unwrap();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "control child process lifecycle".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let worker = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: "run stoppable commands".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        })
        .unwrap();
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    fx.kernel
        .attach_environment(
            &env.environment_id,
            &worker.agent_id,
            &worker.thread_id,
            &fx.task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let command_cap = fx
        .kernel
        .grant_capability(
            &worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let supervisor_approval = fx
        .kernel
        .request_approval(RequestApprovalInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![json!("tool:*")],
                risk_ceiling: 6,
                goal_id: fx.goal.goal_id.clone(),
                task_id: Some(fx.task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    fx.kernel
        .record_approval(RecordApprovalInput {
            approval_id: supervisor_approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "human".to_string(),
            decision_reason: Some("approve supervisor process lifecycle control".to_string()),
        })
        .unwrap();
    let supervisor_cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            6,
            Some(supervisor_approval.approval_id),
        )
        .unwrap();
    let slow_command = if cfg!(windows) {
        "Write-Output before-stop; Start-Sleep -Seconds 30; Write-Output after-stop"
    } else {
        "echo before-stop; sleep 30; echo after-stop"
    };

    let interrupted_command = fx
        .kernel
        .invoke_tool(
            &worker.agent_id,
            &fx.task.task_id,
            &worker.session_id,
            command_cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": slow_command,
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("interruptible command started".to_string()),
            },
        )
        .unwrap();
    assert_eq!(interrupted_command.status, ToolCallStatus::Running);
    let interrupted_process_id = interrupted_command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string();
    wait_process_state(
        &fx.kernel,
        &interrupted_process_id,
        ProcessLifecycleState::Running,
    );
    let listed_running = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "status",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "processes": true,
                        "state": "running"
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    let listed_running_ids = listed_running
        .output
        .as_ref()
        .and_then(|output| output.pointer("/processes/items"))
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|process| {
            process
                .get("process_id")
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Vec<_>>();
    assert!(listed_running_ids.contains(&interrupted_process_id.as_str()));

    let inspected_process = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "status",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "process_id": interrupted_process_id.clone()
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(
        inspected_process
            .output
            .as_ref()
            .and_then(|output| output.pointer("/process/process_id"))
            .and_then(serde_json::Value::as_str),
        Some(interrupted_process_id.as_str())
    );

    let stopped = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id.clone(),
            4,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "stop",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "process_id": interrupted_process_id
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(stopped.status, ToolCallStatus::Completed);
    assert_eq!(
        stopped
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/interrupted"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    wait_process_state(
        &fx.kernel,
        &interrupted_process_id,
        ProcessLifecycleState::Interrupted,
    );

    let terminated_command = fx
        .kernel
        .invoke_tool(
            &worker.agent_id,
            &fx.task.task_id,
            &worker.session_id,
            command_cap.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": slow_command,
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("terminable command started".to_string()),
            },
        )
        .unwrap();
    assert_eq!(terminated_command.status, ToolCallStatus::Running);
    let terminated_process_id = terminated_command
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string();
    wait_process_state(
        &fx.kernel,
        &terminated_process_id,
        ProcessLifecycleState::Running,
    );

    let killed = fx
        .kernel
        .invoke_tool(
            &supervisor.agent_id,
            &fx.task.task_id,
            &supervisor.session_id,
            supervisor_cap.capability_id,
            6,
            ToolInvokeInput {
                tool_name: "agent_control".to_string(),
                input: json!({
                    "action": "kill",
                    "thread_id": worker.thread_id,
                    "payload": {
                        "process_id": terminated_process_id
                    }
                }),
                evidence_claim: None,
            },
        )
        .unwrap();
    assert_eq!(killed.status, ToolCallStatus::Completed);
    assert_eq!(
        killed
            .output
            .as_ref()
            .and_then(|output| output.pointer("/output/terminated"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    wait_process_state(
        &fx.kernel,
        &terminated_process_id,
        ProcessLifecycleState::Terminated,
    );

    let replayed = Kernel::from_events(&fx.kernel.events().unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert_eq!(
        replayed_state
            .process_sessions
            .get(&interrupted_process_id)
            .unwrap()
            .state,
        ProcessLifecycleState::Interrupted
    );
    assert_eq!(
        replayed_state
            .process_sessions
            .get(&terminated_process_id)
            .unwrap()
            .state,
        ProcessLifecycleState::Terminated
    );

    let _ = std::fs::remove_dir_all(workspace);
}

fn wait_process_state(
    kernel: &Kernel,
    process_id: &str,
    expected: ProcessLifecycleState,
) -> ProcessSession {
    let started = std::time::Instant::now();
    loop {
        let session = kernel
            .state_snapshot()
            .unwrap()
            .process_sessions
            .get(process_id)
            .unwrap()
            .clone();
        if session.state == expected {
            return session;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "process {process_id} remained in {:?}, expected {:?}",
            session.state,
            expected
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn default_tool_registry_is_minimal() {
    let fx = fixture();
    let state = fx.kernel.state_snapshot().unwrap();
    let tool_names = state
        .tool_descriptors
        .values()
        .map(|tool| tool.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        tool_names,
        std::collections::BTreeSet::from([
            "apply_patch",
            "glob_files",
            "grep_files",
            "read_file",
            "read_image",
            "run_command",
            "write_stdin",
            "set_goal",
            "accomplish_goal",
            "update_checklist",
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "request_permissions",
            "load_skill",
            "read_skill_resource",
            "tool_search",
            "agent_control",
            "submit_final",
        ])
    );
    let run_command = state.tool_descriptors.get("run_command").unwrap();
    assert_eq!(
        run_command.lifecycle.foreground_timeout_ms,
        DEFAULT_TOOL_FOREGROUND_TIMEOUT_MS
    );
    assert_eq!(
        run_command.lifecycle.background_execution,
        ToolBackgroundExecution::KernelWorker
    );
    assert_eq!(
        run_command.lifecycle.recovery,
        ToolRecoveryPolicy::CancelOrphanRunning
    );
    assert_eq!(
        run_command.lifecycle.output_management.mode,
        ToolOutputManagementMode::ManagedTextFields
    );
    assert_eq!(
        run_command.lifecycle.output_management.default_new_lines,
        TOOL_OUTPUT_DEFAULT_NEW_LINES
    );
    assert_eq!(
        run_command.lifecycle.output_management.default_page_lines,
        TOOL_OUTPUT_DEFAULT_PAGE_LINES
    );
    assert_eq!(
        run_command.lifecycle.output_management.max_lines,
        TOOL_OUTPUT_MAX_LINES
    );
    assert_eq!(
        run_command.lifecycle.output_management.max_window_bytes,
        TOOL_OUTPUT_MAX_WINDOW_BYTES
    );
}

#[test]
fn kernel_tool_plan_projects_direct_hidden_and_disabled_tools() {
    let fx = fixture();
    fx.kernel
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
    let text_only = ModelCapabilities {
        tool_calling: true,
        ..ModelCapabilities::default()
    };
    let producer_plan = fx
        .kernel
        .plan_tools_for_turn(&fx.worker, text_only, ToolPlanningMode::Normal)
        .unwrap();
    let direct_names = producer_plan
        .direct_descriptors()
        .into_iter()
        .map(|descriptor| descriptor.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(direct_names.contains("glob_files"));
    assert!(direct_names.contains("grep_files"));
    assert!(direct_names.contains("submit_final"));
    assert!(direct_names.contains("tool_search"));
    assert!(!direct_names.contains("mcp__echo__echo"));
    assert!(!direct_names.contains("agent_control"));
    assert!(!direct_names.contains("set_goal"));
    let mcp_echo = producer_plan
        .entries
        .iter()
        .find(|entry| entry.descriptor.name == "mcp__echo__echo")
        .unwrap();
    assert_eq!(mcp_echo.exposure, ToolExposure::Deferred);
    assert!(mcp_echo.reason.as_deref().unwrap().contains("tool_search"));
    let read_image = producer_plan
        .entries
        .iter()
        .find(|entry| entry.descriptor.name == "read_image")
        .unwrap();
    assert_eq!(read_image.exposure, ToolExposure::Disabled);
    assert!(read_image
        .reason
        .as_deref()
        .unwrap()
        .contains("image_input"));
    let agent_control = producer_plan
        .entries
        .iter()
        .find(|entry| entry.descriptor.name == "agent_control")
        .unwrap();
    assert_eq!(agent_control.exposure, ToolExposure::Disabled);
    let reason = agent_control.reason.as_deref().unwrap();
    assert!(!reason.is_empty());

    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "integration-test".to_string(),
            goal: "inspect tool plan".to_string(),
            success_criteria: vec!["control plane tools are planned".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let normal_supervisor_plan = fx
        .kernel
        .plan_tools_for_turn(
            &supervisor,
            ModelCapabilities {
                image_input: true,
                tool_calling: true,
                ..ModelCapabilities::default()
            },
            ToolPlanningMode::Normal,
        )
        .unwrap();
    let normal_supervisor_direct = normal_supervisor_plan
        .direct_descriptors()
        .into_iter()
        .map(|descriptor| descriptor.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(normal_supervisor_direct.contains("agent_control"));
    assert!(normal_supervisor_direct.contains("set_goal"));
    let supervisor_plan = fx
        .kernel
        .plan_tools_for_turn(
            &supervisor,
            ModelCapabilities {
                image_input: true,
                tool_calling: true,
                ..ModelCapabilities::default()
            },
            ToolPlanningMode::FinalizationOnly,
        )
        .unwrap();
    let direct_names = supervisor_plan
        .direct_descriptors()
        .into_iter()
        .map(|descriptor| descriptor.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        direct_names,
        std::collections::BTreeSet::from([
            "accomplish_goal".to_string(),
            "submit_final".to_string()
        ])
    );
    let apply_patch = supervisor_plan
        .entries
        .iter()
        .find(|entry| entry.descriptor.name == "apply_patch")
        .unwrap();
    assert_eq!(apply_patch.exposure, ToolExposure::Hidden);

    let events = fx.kernel.events().unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == "ToolPlanCreated" && event.aggregate_id == producer_plan.plan_id
    }));
    let replayed = Kernel::from_events(&events).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let replayed_plan = replayed_state
        .tool_plans
        .get(&producer_plan.plan_id)
        .unwrap();
    assert_eq!(replayed_plan.thread_id, fx.worker.thread_id);
    assert_eq!(replayed_plan.agent_id, fx.worker.agent_id);
    assert_eq!(replayed_plan.task_id, fx.task.task_id);
    assert_eq!(replayed_plan.mode, ToolPlanningMode::Normal);
    assert!(replayed_plan
        .entries
        .iter()
        .any(|entry| entry.descriptor.name == "tool_search"
            && entry.exposure == ToolExposure::Direct));
    assert!(replayed_plan
        .entries
        .iter()
        .any(|entry| entry.descriptor.name == "mcp__echo__echo"
            && entry.exposure == ToolExposure::Deferred));
    assert!(replayed_plan
        .entries
        .iter()
        .any(|entry| entry.descriptor.name == "read_image"
            && entry.exposure == ToolExposure::Disabled));
}

#[test]
fn tool_search_returns_deferred_tool_summaries() {
    let fx = fixture();
    for (tool_name, description, scope, risk_level) in [
        (
            "mcp__alpha__echo",
            "Echo alpha text through MCP.",
            "mcp:alpha:echo",
            2,
        ),
        (
            "mcp__beta__echo",
            "Echo beta text through MCP.",
            "mcp:beta:echo",
            3,
        ),
        (
            "mcp__gamma__echo",
            "Echo gamma text through MCP.",
            "mcp:gamma:echo",
            1,
        ),
    ] {
        fx.kernel
            .register_tool_descriptor(ToolDescriptor {
                tool_id: format!("tool_{tool_name}"),
                name: tool_name.to_string(),
                description: description.to_string(),
                version: "0.3.0".to_string(),
                driver_class: ToolDriverClass::Mcp,
                risk_level,
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
                    required_resource_scopes: vec![scope.to_string()],
                    ..ToolRuntimeInputPolicy::default()
                },
                idempotency: IdempotencyMode::ToolNative,
                evidence_type: Some(EvidenceType::ExternalReference),
                created_at: now_rfc3339(),
                ..ToolDescriptor::default()
            })
            .unwrap();
    }
    let capability = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            3,
            None,
        )
        .unwrap();

    let limited = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            capability.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "tool_search".to_string(),
                input: json!({"query": "echo", "limit": 2}),
                evidence_claim: Some("deferred tool discovery was queried".to_string()),
            },
        )
        .unwrap();

    let output = limited.output.unwrap();
    assert_eq!(output["status"], json!("ok"));
    assert_eq!(output["total_matches"], json!(3));
    assert_eq!(output["returned_matches"], json!(2));
    assert_eq!(output["matches"][0]["name"], json!("mcp__alpha__echo"));
    assert_eq!(output["matches"][1]["name"], json!("mcp__beta__echo"));
    assert_eq!(output["matches"][0]["driver_class"], json!("mcp"));

    let default_limit = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            capability.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "tool_search".to_string(),
                input: json!({"query": "echo"}),
                evidence_claim: Some("deferred tool discovery used the default limit".to_string()),
            },
        )
        .unwrap()
        .output
        .unwrap();
    assert_eq!(default_limit["total_matches"], json!(3));
    assert_eq!(default_limit["returned_matches"], json!(3));

    let multi_word = fx
        .kernel
        .invoke_tool(
            &fx.worker.agent_id,
            &fx.task.task_id,
            &fx.worker.session_id,
            capability.capability_id.clone(),
            1,
            ToolInvokeInput {
                tool_name: "tool_search".to_string(),
                input: json!({"query": "gamma echo"}),
                evidence_claim: Some("multi-word deferred tool discovery was queried".to_string()),
            },
        )
        .unwrap()
        .output
        .unwrap();
    assert_eq!(multi_word["total_matches"], json!(1));
    assert_eq!(multi_word["matches"][0]["name"], json!("mcp__gamma__echo"));

    let invalid_limit = fx.kernel.invoke_tool(
        &fx.worker.agent_id,
        &fx.task.task_id,
        &fx.worker.session_id,
        capability.capability_id,
        1,
        ToolInvokeInput {
            tool_name: "tool_search".to_string(),
            input: json!({"query": "echo", "limit": 0}),
            evidence_claim: Some("invalid tool_search limit was rejected".to_string()),
        },
    );
    let invalid_limit = invalid_limit.unwrap();
    assert_eq!(invalid_limit.status, ToolCallStatus::Failed);
    assert_eq!(
        invalid_limit
            .output
            .as_ref()
            .and_then(|output| output.get("stage")),
        Some(&json!("input_schema"))
    );
    let error = invalid_limit
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(error.contains("tool.input.limit must be >= 1"), "{error}");
}

#[test]
fn tool_search_parameter_failures_are_model_visible_without_evidence() {
    let fx = fixture();
    let capability = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            3,
            None,
        )
        .unwrap();

    for (input, expected_error) in [
        (json!({}), "tool.input missing required field query"),
        (json!({"query": 7}), "tool.input.query expected string"),
        (
            json!({"query": "echo", "limit": "2"}),
            "tool.input.limit expected integer",
        ),
        (
            json!({"query": "echo", "limit": 26}),
            "tool.input.limit must be <= 25",
        ),
    ] {
        let invocation = fx
            .kernel
            .invoke_tool(
                &fx.worker.agent_id,
                &fx.task.task_id,
                &fx.worker.session_id,
                capability.capability_id.clone(),
                1,
                ToolInvokeInput {
                    tool_name: "tool_search".to_string(),
                    input,
                    evidence_claim: Some(
                        "invalid tool_search parameters were rejected".to_string(),
                    ),
                },
            )
            .unwrap();

        assert_eq!(invocation.status, ToolCallStatus::Failed);
        assert!(invocation.evidence_ids.is_empty());
        assert_eq!(
            invocation
                .output
                .as_ref()
                .and_then(|output| output.get("status")),
            Some(&json!("failed"))
        );
        assert_eq!(
            invocation
                .output
                .as_ref()
                .and_then(|output| output.get("stage")),
            Some(&json!("input_schema"))
        );
        let error = invocation
            .output
            .as_ref()
            .and_then(|output| output.get("error"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(error.contains(expected_error), "{error}");
    }

    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state.evidence.is_empty());
}

#[test]
fn tool_invocation_rejects_input_schema_violations_before_side_effects() {
    let fx = fixture();
    let cap = fx
        .kernel
        .grant_capability(
            &fx.worker.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            4,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "tool.invoke",
        fx.worker.agent_id.clone(),
        fx.task.task_id.clone(),
        fx.worker.session_id.clone(),
        Some(cap.capability_id),
        4,
        json!({
            "tool_name": "run_command",
            "input": {"command": std::env::current_exe().unwrap().to_string_lossy()},
            "evidence_claim": "tests were run"
        }),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    let invocation: ToolInvocation = serde_json::from_value(result.output).unwrap();
    assert_eq!(invocation.status, ToolCallStatus::Failed);
    let error = invocation
        .output
        .as_ref()
        .and_then(|output| output.get("error"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(error.contains("tool.input missing required field cwd"));
    let state = fx.kernel.state_snapshot().unwrap();
    assert_eq!(state.tool_invocations.len(), 1);
    assert!(state.evidence.is_empty());
    assert!(!fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "ToolCallStarted"));
}

#[test]
fn tool_descriptor_registration_rejects_invalid_lifecycle_policy() {
    let fx = fixture();
    let err = fx
        .kernel
        .register_tool_descriptor(ToolDescriptor {
            tool_id: "tool_bad_lifecycle".to_string(),
            name: "bad_lifecycle".to_string(),
            version: "0.3.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({"type": "object"}),
            output_schema: json!({"type": "object"}),
            lifecycle: ToolLifecyclePolicy {
                foreground_timeout_ms: 0,
                ..ToolLifecyclePolicy::default()
            },
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: None,
            created_at: now_rfc3339(),
            ..ToolDescriptor::default()
        })
        .unwrap_err();
    assert!(
        matches!(err, AgentOsError::Validation(ref message) if message.contains("foreground timeout")),
        "{err:?}"
    );
}

#[test]
fn tool_invocation_records_failure_when_output_schema_is_violated() {
    let fx = fixture();
    fx.kernel
        .register_tool_descriptor(ToolDescriptor {
            tool_id: "tool_bad_output".to_string(),
            name: "bad.output".to_string(),
            version: "0.1.0".to_string(),
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": {"type": "string"}
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "required": ["status"],
                "properties": {
                    "status": {"enum": ["not-ok"]}
                }
            }),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::CommandLog),
            created_at: now_rfc3339(),
            ..ToolDescriptor::default()
        })
        .unwrap();
    let supervisor = fx
        .kernel
        .spawn_agent(SpawnAgentInput {
            task_id: fx.task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "tester".to_string(),
            goal: "exercise custom tool schema failure".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: Vec::new(),
        })
        .unwrap();
    let cap = fx
        .kernel
        .grant_capability(
            &supervisor.agent_id,
            &fx.task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            1,
            None,
        )
        .unwrap();
    let syscall = SyscallEnvelope::new(
        "tool.invoke",
        supervisor.agent_id.clone(),
        fx.task.task_id.clone(),
        supervisor.session_id.clone(),
        Some(cap.capability_id),
        1,
        json!({
            "tool_name": "bad.output",
            "input": {"message": "hello"},
            "evidence_claim": "should not attach"
        }),
    );
    let result = fx.kernel.handle_syscall(syscall).unwrap();
    let result_invocation: ToolInvocation = serde_json::from_value(result.output).unwrap();
    assert_eq!(result_invocation.status, ToolCallStatus::Failed);
    let state = fx.kernel.state_snapshot().unwrap();
    let invocation = state.tool_invocations.values().next().unwrap();
    assert_eq!(invocation.status, ToolCallStatus::Failed);
    assert!(state.evidence.is_empty());
    assert!(fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "ToolCallFailed"));
}

#[test]
fn artifact_and_evidence_inline_bytes_are_persisted_as_hash_blobs() {
    let root =
        std::env::temp_dir().join(format!("agent-os-kernel-blob-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let artifacts = LocalBlobStore::new(root.join("artifacts")).unwrap();
    let evidence_blobs = LocalBlobStore::new(root.join("evidence")).unwrap();
    let artifact_reader = artifacts.clone();
    let evidence_reader = evidence_blobs.clone();
    let fx = fixture_with_kernel(Kernel::new().with_blob_stores(artifacts, evidence_blobs));

    let evidence = fx
        .kernel
        .attach_evidence(AttachEvidenceInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: Some(fx.task.task_id.clone()),
            artifact_id: None,
            evidence_type: EvidenceType::CommandLog,
            producer_agent_id: Some(fx.worker.agent_id.clone()),
            claim: Some("command output".to_string()),
            blob_ref: None,
            content_hash: None,
            inline_bytes: Some(b"cargo test output".to_vec()),
            metadata: json!({}),
        })
        .unwrap();
    let artifact = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::TestLog,
            blob_ref: None,
            content_hash: None,
            inline_bytes: Some(b"test log artifact".to_vec()),
            metadata: json!({}),
            evidence_ids: vec![evidence.evidence_id.clone()],
            supersedes: None,
        })
        .unwrap();

    assert!(artifact.blob_ref.as_deref().unwrap().starts_with("sha256:"));
    assert!(evidence.blob_ref.as_deref().unwrap().starts_with("sha256:"));
    assert_eq!(
        artifact_reader
            .get_blob(artifact.blob_ref.as_deref().unwrap())
            .unwrap(),
        b"test log artifact"
    );
    assert_eq!(
        evidence_reader
            .get_blob(evidence.blob_ref.as_deref().unwrap())
            .unwrap(),
        b"cargo test output"
    );
    assert_eq!(artifact.metadata["blob_byte_len"], json!(17));
    assert_eq!(evidence.metadata["blob_byte_len"], json!(17));
    let _ = std::fs::remove_dir_all(root);
}
