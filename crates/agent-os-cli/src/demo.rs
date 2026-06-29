use agent_os_kernel::{Kernel, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
use agent_os_sys::{AgentOsResult, EvidenceType};
use serde_json::{json, Value};

pub(crate) fn run_demo() -> AgentOsResult<Value> {
    let kernel = Kernel::new();
    let goal = kernel.register_goal(RegisterGoalInput {
        namespace: "demo".to_string(),
        created_by: "agent-os-cli".to_string(),
        title: "Demo Agent-OS lifecycle".to_string(),
        description: "Run a single-node kernel lifecycle.".to_string(),
        acceptance_criteria: vec!["events replay into projection".to_string()],
        constraints: Vec::new(),
        risk_level: 1,
        deadline: None,
    })?;
    let task = kernel.spawn_task(SpawnTaskInput {
        goal_id: goal.goal_id.clone(),
        parent_task_id: None,
        title: "Inspect docs".to_string(),
        description: "Task for kernel lifecycle demo.".to_string(),
        depends_on: Vec::new(),
        required_artifact_types: Vec::new(),
        required_evidence_types: vec![EvidenceType::SourceRef],
        priority: 10,
        risk_level: 1,
    })?;
    let agent = kernel.spawn_agent(SpawnAgentInput {
        task_id: task.task_id.clone(),
        role_profile_id: "role_worker".to_string(),
        owner: "agent-os-cli".to_string(),
        goal: "inspect docs".to_string(),
        success_criteria: Vec::new(),
        failure_criteria: Vec::new(),
        parent_thread_id: None,
        workspace_roots: vec![".".to_string()],
    })?;
    let state = kernel.state_snapshot()?;
    Ok(json!({
        "goal_id": goal.goal_id,
        "task_id": task.task_id,
        "thread_id": agent.thread_id,
        "agent_id": agent.agent_id,
        "events": kernel.events()?.len(),
        "profiles": {
            "roles": state.role_profiles.len(),
            "permissions": state.permission_profiles.len(),
            "sandboxes": state.sandbox_profiles.len()
        }
    }))
}
