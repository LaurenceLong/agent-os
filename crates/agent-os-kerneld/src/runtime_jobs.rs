use crate::KernelDaemon;
use agent_os_kernel::Kernel;
use agent_os_sys::{AgentOsResult, EventEnvelope};
use agent_os_thread::RuntimeJobRecord;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

impl KernelDaemon {
    pub(crate) fn with_runtime_jobs(
        kernel: Kernel,
        runtime_jobs: BTreeMap<String, RuntimeJobRecord>,
    ) -> Self {
        Self {
            kernel,
            runtime_jobs: Arc::new(Mutex::new(runtime_jobs)),
            runtime_workers: Arc::new(Mutex::new(BTreeMap::new())),
            runtime_model_config: None,
        }
    }

    pub(crate) fn replay_runtime_jobs(
        kernel: &Kernel,
    ) -> AgentOsResult<BTreeMap<String, RuntimeJobRecord>> {
        let mut runtime_jobs = BTreeMap::new();
        for event in kernel.events()? {
            if event.aggregate_type != "runtime_job" {
                continue;
            }
            let record: RuntimeJobRecord = serde_json::from_value(event.payload)?;
            runtime_jobs.insert(record.runtime_job_id.clone(), record);
        }
        Ok(runtime_jobs)
    }

    pub(crate) fn record_runtime_job_event(
        &self,
        event_type: &str,
        record: &RuntimeJobRecord,
    ) -> AgentOsResult<()> {
        let event = EventEnvelope::new(
            event_type,
            "runtime_job",
            &record.runtime_job_id,
            Some(record.job.agent_thread_id.clone()),
            None,
            None,
            Some(record.job.client_thread_id.clone()),
            serde_json::to_value(record)?,
        );
        let store = self.kernel().store();
        store.append(event.clone())?;
        let ordinal = store.event_ordinal(&event.event_id)?;
        store.project_event(ordinal, &event)?;
        Ok(())
    }
}
