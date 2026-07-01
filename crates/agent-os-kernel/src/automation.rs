use crate::*;
use agent_os_sys::*;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

impl Kernel {
    pub fn create_automation_schedule(
        &self,
        input: CreateAutomationScheduleInput,
    ) -> AgentOsResult<AutomationSchedule> {
        if input.name.trim().is_empty() {
            return Err(AgentOsError::Validation(
                "automation schedule name must not be empty".to_string(),
            ));
        }
        if input.prompt.trim().is_empty() {
            return Err(AgentOsError::Validation(
                "automation schedule prompt must not be empty".to_string(),
            ));
        }
        if input.kind == AutomationScheduleKind::ThreadWakeup && input.target_thread_id.is_none() {
            return Err(AgentOsError::Validation(
                "thread wakeup automation requires target_thread_id".to_string(),
            ));
        }
        let now = now_rfc3339();
        let schedule = AutomationSchedule {
            schedule_id: new_id("auto_sched_"),
            name: input.name,
            kind: input.kind,
            status: AutomationScheduleStatus::Active,
            target_thread_id: input.target_thread_id,
            workspace: input.workspace,
            prompt: input.prompt,
            next_run_at: input.next_run_at,
            interval_seconds: input.interval_seconds,
            created_by_client_id: input.created_by_client_id,
            created_at: now.clone(),
            updated_at: now,
            last_run_at: None,
            payload: input.payload,
        };
        self.emit(
            "AutomationScheduleCreated",
            "automation_schedule",
            &schedule.schedule_id,
            None,
            None,
            None,
            None,
            &schedule,
        )?;
        Ok(schedule)
    }

    pub fn queue_automation_run(
        &self,
        schedule_id: &str,
        scheduled_for: impl Into<String>,
    ) -> AgentOsResult<AutomationRun> {
        let scheduled_for = scheduled_for.into();
        let schedule = self
            .read_state()?
            .automation_schedules
            .get(schedule_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("automation schedule {schedule_id}")))?;
        if schedule.status != AutomationScheduleStatus::Active {
            return Err(AgentOsError::InvalidTransition(format!(
                "automation schedule {schedule_id} is not active"
            )));
        }
        let now = now_rfc3339();
        let run = AutomationRun {
            run_id: new_id("auto_run_"),
            schedule_id: schedule.schedule_id.clone(),
            kind: schedule.kind,
            status: AutomationRunStatus::Queued,
            target_thread_id: schedule.target_thread_id.clone(),
            workspace: schedule.workspace.clone(),
            prompt: schedule.prompt.clone(),
            scheduled_for: scheduled_for.clone(),
            queued_at: now.clone(),
            started_at: None,
            completed_at: None,
            error: None,
            payload: schedule.payload.clone(),
        };
        self.emit(
            "AutomationRunQueued",
            "automation_run",
            &run.run_id,
            None,
            None,
            None,
            Some(schedule.schedule_id.clone()),
            &run,
        )?;

        let mut updated = schedule;
        updated.last_run_at = Some(scheduled_for.clone());
        updated.next_run_at = match updated.interval_seconds {
            Some(interval_seconds) => {
                Some(next_interval_timestamp(&scheduled_for, interval_seconds)?)
            }
            None => None,
        };
        updated.updated_at = now;
        self.emit(
            "AutomationScheduleUpdated",
            "automation_schedule",
            &updated.schedule_id,
            None,
            None,
            None,
            None,
            &updated,
        )?;
        Ok(run)
    }
}

fn next_interval_timestamp(scheduled_for: &str, interval_seconds: u64) -> AgentOsResult<String> {
    let seconds = i64::try_from(interval_seconds).map_err(|_| {
        AgentOsError::Validation("automation interval_seconds exceeds supported range".to_string())
    })?;
    let instant = OffsetDateTime::parse(scheduled_for, &Rfc3339).map_err(|error| {
        AgentOsError::Validation(format!("invalid automation scheduled_for: {error}"))
    })?;
    (instant + Duration::seconds(seconds))
        .format(&Rfc3339)
        .map_err(|error| {
            AgentOsError::Validation(format!("invalid automation next_run_at: {error}"))
        })
}
