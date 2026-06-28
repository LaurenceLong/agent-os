use crate::args::ResumeOptions;
use crate::support::{
    ensure_safe_relative_workspace_path, io_result, open_kernel_from_state_db,
    write_task_bundle_if_requested,
};
use agent_os_kernel::Kernel;
use agent_os_sys::*;
use agent_os_thread::{ExternalProcessModelClient, RuntimeConfig, ThreadRuntime};
use serde_json::{json, Value};
use std::fs;

pub(crate) fn run_resume(options: &ResumeOptions) -> AgentOsResult<Value> {
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    let kernel = open_kernel_from_state_db(&options.state_db)?;
    let before = kernel
        .state_snapshot()?
        .threads
        .get(&options.thread_id)
        .cloned()
        .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", options.thread_id)))?;
    // Reconcile durable state left mid-flight (orphan tool calls, expired
    // leases) before resuming, per the recovery contract in
    // `docs/10-kernel-design/agent-thread-core-module.md:747-770`.
    let reconciliation = kernel.reconcile_thread_recovery(&options.thread_id)?;
    prepare_thread_for_resume(&kernel, &options.thread_id)?;

    let client =
        ExternalProcessModelClient::new(options.model_command.clone(), options.model_args.clone());
    let mut runtime = ThreadRuntime::new(kernel.clone(), options.thread_id.clone(), client);
    let report = runtime.run_to_completion(RuntimeConfig::workspace_write(&options.workspace))?;
    let bundle_path = write_task_bundle_if_requested(
        &kernel,
        &report.task_id,
        &options.workspace,
        &options.bundle_output,
    )?;
    let replayed = Kernel::from_events(&kernel.events()?)?;
    let replayed_state = replayed.state_snapshot()?;
    Ok(json!({
        "status": "completed",
        "state_db": options.state_db.to_string_lossy(),
        "thread_id": report.thread_id,
        "task_id": report.task_id,
        "previous_thread_status": before.status,
        "runtime_status": report.status,
        "bundle_path": bundle_path,
        "provider_stream_session_ids": report.provider_stream_session_ids,
        "tool_results": report.tool_results,
        "artifacts": report.artifacts,
        "events": report.events,
        "replay": {
            "tasks": replayed_state.tasks.len(),
            "threads": replayed_state.threads.len(),
            "artifacts": replayed_state.artifacts.len(),
            "evidence": replayed_state.evidence.len(),
            "final_submissions": replayed_state.final_submissions.len()
        },
        "reconciliation": {
            "reconciliation_id": reconciliation.reconciliation_id,
            "orphan_tool_call_ids": reconciliation.orphan_tool_call_ids,
            "workspace_diff_refs": reconciliation.workspace_diff_refs,
            "reclaimed_resource_lease_ids": reconciliation.reclaimed_resource_lease_ids,
            "reclaimed_environment_lease_ids": reconciliation.reclaimed_environment_lease_ids
        }
    }))
}

fn prepare_thread_for_resume(kernel: &Kernel, thread_id: &str) -> AgentOsResult<()> {
    let status = kernel
        .state_snapshot()?
        .threads
        .get(thread_id)
        .map(|thread| thread.status)
        .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
    match status {
        ThreadStatus::Created | ThreadStatus::Ready => Ok(()),
        ThreadStatus::Running
        | ThreadStatus::WaitingTool
        | ThreadStatus::WaitingPermission
        | ThreadStatus::WaitingUser => {
            kernel.transition_thread(
                thread_id,
                ThreadStatus::Interrupted,
                Some("resume recovered incomplete turn".to_string()),
            )?;
            kernel.transition_thread(
                thread_id,
                ThreadStatus::Ready,
                Some("resume requested after recovery".to_string()),
            )?;
            Ok(())
        }
        ThreadStatus::Interrupted
        | ThreadStatus::Blocked
        | ThreadStatus::Suspended
        | ThreadStatus::ResidentIdle
        | ThreadStatus::Unloaded => {
            kernel.transition_thread(
                thread_id,
                ThreadStatus::Ready,
                Some("resume requested".to_string()),
            )?;
            Ok(())
        }
        ThreadStatus::Completing
        | ThreadStatus::Completed
        | ThreadStatus::Failed
        | ThreadStatus::Quarantined
        | ThreadStatus::Terminated => Err(AgentOsError::InvalidTransition(format!(
            "thread {:?} cannot be resumed",
            status
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_kernel::{RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
    use agent_os_store_sqlite::SqliteStore;
    use std::env;

    #[test]
    fn prepare_thread_for_resume_recovers_running_thread_after_replay() {
        let state_db = env::temp_dir().join(format!(
            "agent-os-cli-resume-{}-{}.sqlite",
            std::process::id(),
            new_id("case_")
        ));
        let kernel = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "resume-test".to_string(),
                created_by: "agent-os-cli-test".to_string(),
                title: "Resume".to_string(),
                description: "Resume".to_string(),
                acceptance_criteria: vec!["thread resumes".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id,
                parent_task_id: None,
                title: "Task".to_string(),
                description: "Task".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 1,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_worker".to_string(),
                owner: "agent-os-cli-test".to_string(),
                local_goal: "Resume".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![".".to_string()],
            })
            .unwrap();
        kernel.start_turn(&agent.thread_id).unwrap();

        let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
        prepare_thread_for_resume(&replayed, &agent.thread_id).unwrap();
        let state = replayed.state_snapshot().unwrap();
        assert_eq!(
            state.threads.get(&agent.thread_id).unwrap().status,
            ThreadStatus::Ready
        );
        let _ = std::fs::remove_file(state_db);
    }
}
