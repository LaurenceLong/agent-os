mod selection;
mod snapshot;
mod types;

use crate::*;
use agent_os_sys::*;
use selection::{collect_bundle_selection, task_subtree_ids};
use snapshot::{filter_events, profile_snapshot, projection_snapshot};
use types::BundleSelection;

pub use types::*;

impl Kernel {
    pub fn export_task_bundle(&self, task_id: &str) -> AgentOsResult<TaskBundle> {
        self.export_bundle(task_id, BundleKind::Task)
    }

    pub fn export_replay_bundle(&self, task_id: &str) -> AgentOsResult<TaskBundle> {
        self.export_bundle(task_id, BundleKind::Replay)
    }

    fn export_bundle(&self, task_id: &str, bundle_kind: BundleKind) -> AgentOsResult<TaskBundle> {
        let state = self.state_snapshot()?;
        let root_task = state
            .tasks
            .get(task_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("task {task_id}")))?;
        let goal = state
            .goals
            .get(&root_task.goal_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("goal {}", root_task.goal_id)))?;
        let mut selection = BundleSelection {
            goal_id: root_task.goal_id.clone(),
            task_ids: task_subtree_ids(&state, task_id),
            ..BundleSelection::default()
        };
        collect_bundle_selection(&state, &mut selection);
        let profile_snapshot = profile_snapshot(&state, &selection);
        let projection_snapshot = projection_snapshot(&state, &selection, goal)?;
        let events = filter_events(self.events()?, &selection);
        let task_ids = selection.task_ids.iter().cloned().collect();
        let replay_summary = TaskBundleReplaySummary {
            event_count: events.len(),
            task_count: projection_snapshot.tasks.len(),
            thread_count: projection_snapshot.threads.len(),
            artifact_count: projection_snapshot.artifacts.len(),
            evidence_count: projection_snapshot.evidence.len(),
            final_submission_count: projection_snapshot.final_submissions.len(),
        };
        Ok(TaskBundle {
            abi_version: ABI_VERSION.to_string(),
            bundle_kind,
            exported_at: now_rfc3339(),
            root_task_id: task_id.to_string(),
            goal_id: selection.goal_id,
            task_ids,
            profile_snapshot,
            projection_snapshot,
            events,
            replay_summary,
        })
    }
}
