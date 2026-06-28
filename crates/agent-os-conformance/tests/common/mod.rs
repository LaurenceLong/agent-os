#![allow(dead_code)]

pub use agent_os_kernel::*;
pub use agent_os_sys::*;
pub use serde_json::json;

pub struct Fixture {
    pub kernel: Kernel,
    pub goal: Goal,
    pub task: Task,
    pub worker: AgentControlBlock,
}

pub fn fixture() -> Fixture {
    fixture_with_kernel(Kernel::new())
}

pub fn fixture_with_kernel(kernel: Kernel) -> Fixture {
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "conformance".to_string(),
            created_by: "tester".to_string(),
            title: "Implement kernel".to_string(),
            description: "Exercise contracts".to_string(),
            acceptance_criteria: vec!["state is replayable".to_string()],
            constraints: Vec::new(),
            risk_level: 3,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Patch".to_string(),
            description: "Produce patch".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: vec![ArtifactType::Patch],
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 3,
        })
        .unwrap();
    let worker = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_worker".to_string(),
            owner: "tester".to_string(),
            local_goal: "write patch".to_string(),
            success_criteria: vec!["patch has diff evidence".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![".".to_string()],
        })
        .unwrap();
    Fixture {
        kernel,
        goal,
        task,
        worker,
    }
}

pub fn evidence_input(fx: &Fixture, evidence_type: EvidenceType) -> AttachEvidenceInput {
    AttachEvidenceInput {
        goal_id: fx.goal.goal_id.clone(),
        task_id: Some(fx.task.task_id.clone()),
        artifact_id: None,
        evidence_type,
        producer_agent_id: Some(fx.worker.agent_id.clone()),
        claim: Some("claim".to_string()),
        blob_ref: Some("blob://demo".to_string()),
        content_hash: Some("hash".to_string()),
        inline_bytes: None,
        metadata: json!({"source": "conformance"}),
    }
}

pub fn attach_writable_environment(fx: &Fixture) -> EnvironmentLease {
    let env = fx
        .kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            "rust-workspace",
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
        .unwrap()
}
