use super::*;

pub(super) fn make_request(workspace: &std::path::Path) -> ModelTurnRequest {
    let (_kernel, request) = make_kernel_request(workspace);
    request
}

pub(super) fn make_kernel_request(workspace: &std::path::Path) -> (Kernel, ModelTurnRequest) {
    make_kernel_request_for_role(
        workspace,
        "role_worker",
        "Write hello world to file",
        vec!["output file exists".to_string()],
    )
}

pub(super) fn make_kernel_request_for_role(
    workspace: &std::path::Path,
    role_profile_id: &str,
    local_goal: &str,
    success_criteria: Vec<String>,
) -> (Kernel, ModelTurnRequest) {
    make_kernel_request_for_role_on_kernel(
        Kernel::new(),
        workspace,
        role_profile_id,
        local_goal,
        success_criteria,
    )
}

pub(super) fn make_kernel_request_for_role_with_blob_store_and_requirements(
    workspace: &std::path::Path,
    role_profile_id: &str,
    local_goal: &str,
    success_criteria: Vec<String>,
    required_artifact_types: Vec<ArtifactType>,
    required_evidence_types: Vec<EvidenceType>,
) -> (Kernel, ModelTurnRequest) {
    let artifact_blobs =
        LocalBlobStore::new(workspace.join(".agent-os-blobs").join("artifacts")).unwrap();
    let evidence_blobs =
        LocalBlobStore::new(workspace.join(".agent-os-blobs").join("evidence")).unwrap();
    make_kernel_request_for_role_on_kernel_with_requirements(
        Kernel::new().with_blob_stores(artifact_blobs, evidence_blobs),
        workspace,
        role_profile_id,
        local_goal,
        success_criteria,
        required_artifact_types,
        required_evidence_types,
    )
}

pub(super) fn make_kernel_request_for_role_on_kernel(
    kernel: Kernel,
    workspace: &std::path::Path,
    role_profile_id: &str,
    local_goal: &str,
    success_criteria: Vec<String>,
) -> (Kernel, ModelTurnRequest) {
    make_kernel_request_for_role_on_kernel_with_requirements(
        kernel,
        workspace,
        role_profile_id,
        local_goal,
        success_criteria,
        vec![ArtifactType::Patch],
        vec![EvidenceType::DiffRef],
    )
}

pub(super) fn make_kernel_request_for_role_on_kernel_with_requirements(
    kernel: Kernel,
    workspace: &std::path::Path,
    role_profile_id: &str,
    local_goal: &str,
    success_criteria: Vec<String>,
    required_artifact_types: Vec<ArtifactType>,
    required_evidence_types: Vec<EvidenceType>,
) -> (Kernel, ModelTurnRequest) {
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "test".to_string(),
            created_by: "test".to_string(),
            title: "Test task".to_string(),
            description: "Test".to_string(),
            acceptance_criteria: vec!["file exists".to_string()],
            constraints: Vec::new(),
            risk_level: 3,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Test".to_string(),
            description: "Test".to_string(),
            depends_on: Vec::new(),
            required_artifact_types,
            required_evidence_types,
            priority: 10,
            risk_level: 3,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: role_profile_id.to_string(),
            owner: "test".to_string(),
            local_goal: local_goal.to_string(),
            success_criteria,
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let request = ModelTurnRequest {
        thread: agent,
        workspace_root: workspace.to_path_buf(),
        step_index: 0,
        tool_results: Vec::new(),
        artifacts: Vec::new(),
    };
    (kernel, request)
}

pub(super) fn attach_workspace_and_grant(
    kernel: &Kernel,
    request: &ModelTurnRequest,
    risk_ceiling: u8,
) -> CapabilityToken {
    let agent = &request.thread;
    let task_id = &agent.task.task_id;
    let env = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            request.workspace_root.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &env.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    kernel
        .grant_capability(
            &agent.agent_id,
            task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:*".to_string()],
            risk_ceiling,
            None,
        )
        .unwrap()
}

pub(super) fn execute_tool_actions(
    kernel: &Kernel,
    request: &ModelTurnRequest,
    capability: &CapabilityToken,
    actions: Vec<ModelAction>,
) -> Vec<ToolExecutionRecord> {
    let agent = &request.thread;
    let task_id = &agent.task.task_id;
    let mut records = Vec::new();
    for action in actions {
        let ModelAction::ToolCall(action) = action else {
            continue;
        };
        let invocation = kernel
            .invoke_tool(
                &agent.agent_id,
                task_id,
                &agent.session_id,
                capability.capability_id.clone(),
                action.risk_level,
                ToolInvokeInput {
                    tool_name: action.tool_name,
                    input: action.input,
                    evidence_claim: action.evidence_claim.clone(),
                },
            )
            .unwrap();
        records.push(ToolExecutionRecord {
            call_id: invocation.call_id,
            tool_name: invocation.tool_name,
            status: invocation.status,
            input: Some(invocation.input),
            output: invocation.output,
            evidence_ids: invocation.evidence_ids,
            evidence_claim: action.evidence_claim,
        });
    }
    records
}

pub(super) fn assert_core_tool_e2e_effects(
    workspace: &std::path::Path,
    records: &[ToolExecutionRecord],
) {
    assert_eq!(
        std::fs::read_to_string(workspace.join("created.txt")).unwrap(),
        "created by provider mock\n"
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("edit.txt")).unwrap(),
        "alpha new beta\n"
    );
    assert!(!workspace.join("delete.txt").exists());
    for expected in [
        "read_file",
        "write_file",
        "replace_text",
        "delete_file",
        "run_command",
        "set_objective",
        "update_checklist",
        "record_evidence",
        "report_supervisor",
        "post_blackboard",
        "ask_human",
        "agent_control",
    ] {
        assert!(
            records.iter().any(|record| record.tool_name == expected),
            "missing executed tool {expected}"
        );
    }
    let output_for = |tool_name: &str| {
        records
            .iter()
            .find(|record| record.tool_name == tool_name)
            .and_then(|record| record.output.as_ref())
            .unwrap_or_else(|| panic!("missing output for {tool_name}"))
    };
    assert_eq!(
        output_for("set_objective")["objective"],
        "complete provider-neutral all-tool e2e"
    );
    assert_eq!(
        output_for("update_checklist")["items"][0]["status"],
        "completed"
    );
    assert!(output_for("record_evidence")["evidence_id"]
        .as_str()
        .unwrap()
        .starts_with("evd_"));
    assert_eq!(
        output_for("report_supervisor")["delivery_status"],
        "Delivered"
    );
    assert!(output_for("post_blackboard")["entry_id"]
        .as_str()
        .unwrap()
        .starts_with("bb_"));
    assert_eq!(output_for("ask_human")["delivery_status"], "Delivered");
}

pub(super) fn write_e2e_interaction_log(file_name: &str, entries: &[Value]) -> std::path::PathBuf {
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(file_name);
    std::fs::create_dir_all(audit_log_path.parent().unwrap()).unwrap();
    let mut audit_log = std::fs::File::create(&audit_log_path).unwrap();
    for entry in entries {
        use std::io::Write;
        writeln!(audit_log, "{}", serde_json::to_string(entry).unwrap()).unwrap();
    }
    println!("e2e_interaction_log={}", audit_log_path.display());
    audit_log_path
}
