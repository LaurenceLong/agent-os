use crate::AgentOsHost;
use agent_os_app_server::{thread_read_response, ThreadReadProjection};
use agent_os_sys::{AgentOsResult, AppResponse};

impl AgentOsHost {
    pub(crate) fn thread_read_projection(
        &self,
        client_thread_id: &str,
    ) -> AgentOsResult<AppResponse> {
        let thread = self.thread_by_id(client_thread_id)?;
        let turns = self
            .kernel()
            .store()
            .turn_summaries()?
            .into_iter()
            .filter(|turn| turn.client_thread_id.as_deref() == Some(client_thread_id))
            .collect::<Vec<_>>();
        let timeline = self
            .kernel()
            .store()
            .timeline_items(Some(client_thread_id))?;
        let runtime_jobs = self.runtime_jobs_for_thread(client_thread_id)?;
        let mut process_sessions = self
            .kernel()
            .state_snapshot()?
            .process_sessions
            .into_values()
            .filter(|process| process.thread_id == client_thread_id)
            .collect::<Vec<_>>();
        process_sessions.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then_with(|| left.process_id.cmp(&right.process_id))
        });
        let artifacts = self
            .kernel()
            .store()
            .artifact_index()?
            .into_iter()
            .filter(|artifact| {
                Some(artifact.task_id.as_str()) == thread.task_id.as_deref()
                    || artifact.agent_id == thread.agent_thread_id
            })
            .collect::<Vec<_>>();
        let evidence = self
            .kernel()
            .store()
            .evidence_index()?
            .into_iter()
            .filter(|evidence| {
                Some(evidence.task_id.as_str()) == thread.task_id.as_deref()
                    || evidence.agent_id == thread.agent_thread_id
            })
            .collect::<Vec<_>>();
        let resources = self
            .kernel()
            .store()
            .resource_sessions()?
            .into_iter()
            .filter(|resource| resource.client_thread_id.as_deref() == Some(client_thread_id))
            .collect::<Vec<_>>();
        let automation_runs = self
            .kernel()
            .store()
            .automation_runs()?
            .into_iter()
            .filter(|run| run.target_thread_id.as_deref() == Some(client_thread_id))
            .collect::<Vec<_>>();
        thread_read_response(ThreadReadProjection {
            thread,
            turns,
            timeline,
            runtime_jobs,
            process_sessions,
            artifacts,
            evidence,
            resources,
            automation_runs,
        })
    }
}
