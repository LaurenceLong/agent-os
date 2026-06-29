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
            role_profile_id: "role_worker".to_string(),
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
            supervisor_cap.capability_id,
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
                "program": std::env::current_exe().unwrap().to_string_lossy(),
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
                tool_name: "write_file".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "result.md",
                    "content": "hello"
                }),
                evidence_claim: Some("workspace file was written".to_string()),
            },
        )
        .unwrap_err();
    assert!(matches!(denied, AgentOsError::PermissionDenied(_)));

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
                tool_name: "write_file".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "result.md",
                    "content": "hello"
                }),
                evidence_claim: Some("workspace file was written".to_string()),
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
        "hello"
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
        Some("hello")
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
                tool_name: "replace_text".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "result.md",
                    "old": "hello",
                    "new": "hello from replace"
                }),
                evidence_claim: Some("workspace file was edited".to_string()),
            },
        )
        .unwrap();
    assert_eq!(replace.status, ToolCallStatus::Completed);
    assert_eq!(replace.evidence_ids.len(), 1);
    assert_eq!(
        std::fs::read_to_string(workspace.join("result.md")).unwrap(),
        "hello from replace"
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
                tool_name: "delete_file".to_string(),
                input: json!({
                    "workspace_root": workspace.to_string_lossy(),
                    "path": "result.md"
                }),
                evidence_claim: Some("workspace file was deleted".to_string()),
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
                    "program": std::env::current_exe().unwrap().to_string_lossy(),
                    "args": ["--help"],
                    "cwd": workspace.to_string_lossy()
                }),
                evidence_claim: Some("process tool executed".to_string()),
            },
        )
        .unwrap();
    assert_eq!(process.status, ToolCallStatus::Completed);
    assert_eq!(process.evidence_ids.len(), 1);
    assert_eq!(
        fx.kernel
            .state_snapshot()
            .unwrap()
            .evidence
            .get(&process.evidence_ids[0])
            .unwrap()
            .evidence_type,
        EvidenceType::CommandLog
    );

    let _ = std::fs::remove_dir_all(workspace);
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
            "read_file",
            "write_file",
            "replace_text",
            "delete_file",
            "run_command",
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
            "agent_control",
            "submit_final",
        ])
    );
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
            "input": {"program": std::env::current_exe().unwrap().to_string_lossy()},
            "evidence_claim": "tests were run"
        }),
    );
    let err = fx.kernel.handle_syscall(syscall).unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)), "{err:?}");
    let state = fx.kernel.state_snapshot().unwrap();
    assert!(state.tool_invocations.is_empty());
    assert!(state.evidence.is_empty());
    assert!(!fx
        .kernel
        .events()
        .unwrap()
        .iter()
        .any(|event| event.event_type == "ToolCallStarted"));
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
    let err = fx.kernel.handle_syscall(syscall).unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)), "{err:?}");
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
