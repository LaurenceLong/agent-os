use crate::args::StatusOptions;
use crate::support::open_kernel_from_state_db;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(crate) fn run_status(options: &StatusOptions) -> AgentOsResult<Value> {
    let kernel = open_kernel_from_state_db(&options.state_db)?;
    let state = kernel.state_snapshot()?;
    let mut threads: Vec<_> = state.threads.values().cloned().collect();
    threads.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    if let Some(thread_id) = &options.thread_id {
        let thread = state
            .threads
            .get(thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {thread_id}")))?;
        return Ok(json!({
            "state_db": options.state_db.to_string_lossy(),
            "events": kernel.events()?.len(),
            "thread": thread_summary(thread),
            "task": state.tasks.get(&thread.task.task_id),
            "final_submission": state.final_submissions.get(&thread.task.task_id),
        }));
    }
    Ok(json!({
        "state_db": options.state_db.to_string_lossy(),
        "events": kernel.events()?.len(),
        "goals": state.goals.len(),
        "tasks": state.tasks.len(),
        "threads": threads.iter().map(thread_summary).collect::<Vec<_>>(),
        "artifacts": state.artifacts.len(),
        "evidence": state.evidence.len(),
        "final_submissions": state.final_submissions.len(),
    }))
}

fn thread_summary(thread: &AgentControlBlock) -> Value {
    json!({
        "thread_id": thread.thread_id,
        "agent_id": thread.agent_id,
        "task_id": thread.task.task_id,
        "goal_id": thread.task.goal_id,
        "role": thread.role,
        "status": thread.status,
        "status_reason": thread.status_reason,
        "active_turn": thread.active_turn,
        "last_checkpoint_id": thread.recovery.last_checkpoint_id,
        "workspace_roots": thread.config_snapshot.workspace_roots,
        "goal": thread.task.goal,
    })
}
