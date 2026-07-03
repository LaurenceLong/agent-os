use agent_os_sys::{
    AgentControlBlock, AgentGoalCompletion, AgentOsResult, Approval, ApprovalQueueProjection,
    ApprovalStatus, Artifact, ArtifactIndexProjection, AutomationRun, AutomationRunProjection,
    AutomationSchedule, AutomationScheduleProjection, BudgetLedger, ClientThread,
    ContextCompaction, EventEnvelope, Evidence, EvidenceIndexProjection, ProjectionCheckpoint,
    ProviderStreamSession, ResourceLease, ResourceSession, ResourceSessionProjection,
    StatsSnapshot, ThreadForkRecord, ThreadRollbackRecord, TimelineItem, TimelineItemType,
    ToolCallStatus, ToolInvocation, TurnInputRecord, TurnRecord, TurnStatus,
};
use std::collections::BTreeMap;

const CHECKPOINT_NAMES: [&str; 11] = [
    "thread_summaries",
    "turn_summaries",
    "timeline_items",
    "stats_rollups",
    "approval_queue",
    "resource_sessions",
    "automation_schedules",
    "automation_runs",
    "artifact_index",
    "evidence_index",
    "event_stream",
];

#[derive(Debug, Clone, Default)]
pub struct ProjectionState {
    pub threads: BTreeMap<String, ClientThread>,
    pub turns: BTreeMap<String, TurnRecord>,
    pub timeline_items: Vec<TimelineItem>,
    pub stats: StatsSnapshot,
    pub approvals: BTreeMap<String, ApprovalQueueProjection>,
    pub resources: BTreeMap<String, ResourceSessionProjection>,
    pub automation_schedules: BTreeMap<String, AutomationScheduleProjection>,
    pub automation_runs: BTreeMap<String, AutomationRunProjection>,
    pub artifacts: BTreeMap<String, ArtifactIndexProjection>,
    pub evidence: BTreeMap<String, EvidenceIndexProjection>,
    pub checkpoints: BTreeMap<String, ProjectionCheckpoint>,
}

struct TimelineDraft {
    item_type: TimelineItemType,
    client_thread_id: Option<String>,
    agent_id: Option<String>,
    task_id: Option<String>,
    turn_id: Option<String>,
    summary: String,
}

impl ProjectionState {
    pub fn rebuild(events: &[EventEnvelope]) -> AgentOsResult<Self> {
        let mut state = Self::default();
        for (index, event) in events.iter().enumerate() {
            state.apply_event(index as u64 + 1, event)?;
        }
        Ok(state)
    }

    pub fn apply_event(&mut self, ordinal: u64, event: &EventEnvelope) -> AgentOsResult<()> {
        if self.last_projected_ordinal() >= ordinal {
            return Ok(());
        }
        match event.event_type.as_str() {
            "ThreadConfigured"
            | "ThreadStatusChanged"
            | "AgentStatePurged"
            | "ThreadGoalAccomplished"
            | "TurnStarted"
            | "CheckpointCommitted" => {
                let thread: AgentControlBlock = serde_json::from_value(event.payload.clone())?;
                self.apply_thread(&thread, event);
            }
            "AgentGoalAccomplished" => {
                let completion: AgentGoalCompletion =
                    serde_json::from_value(event.payload.clone())?;
                self.apply_thread(&completion.thread, event);
            }
            "ProviderUsageRecorded" => {
                let session: ProviderStreamSession = serde_json::from_value(event.payload.clone())?;
                self.apply_provider_usage(&session, event);
            }
            "ThreadArchived" | "ThreadUnarchived" | "ThreadDeleted" | "ThreadRenamed" => {
                let thread: ClientThread = serde_json::from_value(event.payload.clone())?;
                self.apply_client_thread_update(thread, event);
            }
            "TurnInputRecorded" => {
                let input: TurnInputRecord = serde_json::from_value(event.payload.clone())?;
                self.apply_turn_input(&input, event);
            }
            "ThreadForked" => {
                let record: ThreadForkRecord = serde_json::from_value(event.payload.clone())?;
                self.apply_thread_fork(&record, event);
            }
            "ThreadRolledBack" => {
                let record: ThreadRollbackRecord = serde_json::from_value(event.payload.clone())?;
                self.apply_thread_rollback(&record, event);
            }
            "ContextCompacted" => {
                let compaction: ContextCompaction = serde_json::from_value(event.payload.clone())?;
                self.apply_context_compaction(&compaction, event);
            }
            "ProviderStreamFailed" | "ProviderStreamCancelled" => {
                self.stats.provider_errors += 1;
                self.touch_stats(event);
            }
            "ToolCallProgressed" | "ToolCallCompleted" | "ToolCallFailed" | "ToolCallDenied"
            | "ToolCallReconciled" => {
                let invocation: ToolInvocation = serde_json::from_value(event.payload.clone())?;
                self.apply_tool_invocation(&invocation, event);
            }
            "ApprovalRequested" | "ApprovalRecorded" => {
                let approval: Approval = serde_json::from_value(event.payload.clone())?;
                self.apply_approval(&approval, event);
            }
            "BudgetDebited" => {
                let _ledger: BudgetLedger = serde_json::from_value(event.payload.clone())?;
                self.stats.budget_debits += 1;
                self.touch_stats(event);
            }
            "ArtifactCommitted" => {
                let artifact: Artifact = serde_json::from_value(event.payload.clone())?;
                self.apply_artifact(&artifact, event);
            }
            "EvidenceAttached" => {
                let evidence: Evidence = serde_json::from_value(event.payload.clone())?;
                self.apply_evidence(&evidence, event);
            }
            "ResourceLeaseGranted"
            | "ResourceLeaseDenied"
            | "ResourceLeaseReleased"
            | "ResourceLeaseReclaimed" => {
                let lease: ResourceLease = serde_json::from_value(event.payload.clone())?;
                self.apply_resource_lease(&lease, event);
            }
            "ResourceSessionOpened" | "ResourceSessionClosed" => {
                let session: ResourceSession = serde_json::from_value(event.payload.clone())?;
                self.apply_resource_session(&session, event);
            }
            "AutomationScheduleCreated" | "AutomationScheduleUpdated" => {
                let schedule: AutomationSchedule = serde_json::from_value(event.payload.clone())?;
                self.apply_automation_schedule(&schedule);
            }
            "AutomationRunQueued"
            | "AutomationRunStarted"
            | "AutomationRunCompleted"
            | "AutomationRunFailed"
            | "AutomationRunCancelled" => {
                let run: AutomationRun = serde_json::from_value(event.payload.clone())?;
                self.apply_automation_run(&run);
            }
            _ => {}
        }
        self.update_checkpoints(ordinal, &event.created_at);
        Ok(())
    }

    fn apply_thread(&mut self, thread: &AgentControlBlock, event: &EventEnvelope) {
        let active_turn_id = thread.active_turn.turn_id.clone();
        let existing = self.threads.get(&thread.thread_id);
        let client_thread = ClientThread {
            client_thread_id: thread.thread_id.clone(),
            agent_thread_id: thread.thread_id.clone(),
            task_id: Some(thread.task.task_id.clone()),
            goal_id: Some(thread.task.goal_id.clone()),
            title: existing
                .map(|thread| thread.title.clone())
                .unwrap_or_else(|| thread.task.goal.clone()),
            status: thread.status,
            active_turn_id: active_turn_id.clone(),
            archived: existing.map(|thread| thread.archived).unwrap_or(false),
            deleted: existing.map(|thread| thread.deleted).unwrap_or(false),
            updated_at: event.created_at.clone(),
        };
        self.threads
            .insert(client_thread.client_thread_id.clone(), client_thread);
        if event.event_type == "TurnStarted" {
            if let Some(turn_id) = &active_turn_id {
                let turn = TurnRecord {
                    turn_id: turn_id.clone(),
                    client_thread_id: Some(thread.thread_id.clone()),
                    agent_thread_id: thread.thread_id.clone(),
                    task_id: Some(thread.task.task_id.clone()),
                    goal_id: Some(thread.task.goal_id.clone()),
                    status: thread.active_turn.status.unwrap_or(TurnStatus::InProgress),
                    started_at: thread
                        .active_turn
                        .started_at
                        .clone()
                        .unwrap_or_else(|| event.created_at.clone()),
                    completed_at: None,
                };
                self.turns.insert(turn.turn_id.clone(), turn);
            }
        }
        if let Some(turn_id) = &active_turn_id {
            if let Some(turn) = self.turns.get_mut(turn_id) {
                if let Some(status) = thread.active_turn.status {
                    turn.status = status;
                    if matches!(
                        status,
                        TurnStatus::Completed
                            | TurnStatus::Failed
                            | TurnStatus::Interrupted
                            | TurnStatus::Blocked
                    ) {
                        turn.completed_at = Some(event.created_at.clone());
                    }
                }
            }
        }
        let item_type = if event.event_type == "TurnStarted" {
            TimelineItemType::TurnStarted
        } else {
            TimelineItemType::ThreadChanged
        };
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type,
                client_thread_id: Some(thread.thread_id.clone()),
                agent_id: Some(thread.agent_id.clone()),
                task_id: Some(thread.task.task_id.clone()),
                turn_id: active_turn_id,
                summary: format!("{} {}", event.event_type, thread.thread_id),
            },
        );
    }

    fn apply_client_thread_update(&mut self, mut thread: ClientThread, event: &EventEnvelope) {
        thread.updated_at = event.created_at.clone();
        self.threads
            .insert(thread.client_thread_id.clone(), thread.clone());
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ThreadChanged,
                client_thread_id: Some(thread.client_thread_id.clone()),
                agent_id: Some(thread.agent_thread_id.clone()),
                task_id: thread.task_id.clone(),
                turn_id: thread.active_turn_id.clone(),
                summary: format!("{} {}", event.event_type, thread.client_thread_id),
            },
        );
    }

    fn apply_turn_input(&mut self, input: &TurnInputRecord, event: &EventEnvelope) {
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::AgentMessageDelta,
                client_thread_id: Some(input.client_thread_id.clone()),
                agent_id: None,
                task_id: event.task_id.clone(),
                turn_id: Some(input.turn_id.clone()),
                summary: format!("turn input {:?}", input.kind),
            },
        );
    }

    fn apply_thread_fork(&mut self, record: &ThreadForkRecord, event: &EventEnvelope) {
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ThreadChanged,
                client_thread_id: Some(record.source_thread_id.clone()),
                agent_id: event.agent_id.clone(),
                task_id: event.task_id.clone(),
                turn_id: record.from_turn_id.clone(),
                summary: format!(
                    "thread {} forked to {}",
                    record.source_thread_id, record.forked_thread_id
                ),
            },
        );
    }

    fn apply_thread_rollback(&mut self, record: &ThreadRollbackRecord, event: &EventEnvelope) {
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ThreadChanged,
                client_thread_id: Some(record.thread_id.clone()),
                agent_id: event.agent_id.clone(),
                task_id: event.task_id.clone(),
                turn_id: record.target_turn_id.clone(),
                summary: format!("thread {} rolled back", record.thread_id),
            },
        );
    }

    fn apply_context_compaction(&mut self, compaction: &ContextCompaction, event: &EventEnvelope) {
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ThreadChanged,
                client_thread_id: Some(compaction.thread_id.clone()),
                agent_id: Some(compaction.agent_id.clone()),
                task_id: Some(compaction.task_id.clone()),
                turn_id: None,
                summary: format!("context compacted {}", compaction.compaction_id),
            },
        );
    }

    fn apply_provider_usage(&mut self, session: &ProviderStreamSession, event: &EventEnvelope) {
        self.stats.input_tokens += session.usage.input_tokens;
        self.stats.output_tokens += session.usage.output_tokens;
        self.stats.cost += session.usage.cost;
        self.stats.provider_calls += 1;
        self.touch_stats(event);
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::StatsUpdated,
                client_thread_id: Some(session.request.thread_id.clone()),
                agent_id: Some(session.request.thread_id.clone()),
                task_id: Some(session.request.task_id.clone()),
                turn_id: session.request.turn_id.clone(),
                summary: format!("provider usage {}", session.session_id),
            },
        );
    }

    fn apply_tool_invocation(&mut self, invocation: &ToolInvocation, event: &EventEnvelope) {
        if matches!(
            invocation.status,
            ToolCallStatus::Completed
                | ToolCallStatus::Failed
                | ToolCallStatus::Denied
                | ToolCallStatus::Cancelled
                | ToolCallStatus::TimedOut
        ) {
            self.stats.tool_calls += 1;
        }
        match invocation.status {
            ToolCallStatus::Completed => self.stats.tool_successes += 1,
            ToolCallStatus::Failed | ToolCallStatus::Cancelled | ToolCallStatus::TimedOut => {
                self.stats.tool_failures += 1;
            }
            ToolCallStatus::Denied => self.stats.tool_denials += 1,
            ToolCallStatus::Proposed
            | ToolCallStatus::Validated
            | ToolCallStatus::PendingApproval
            | ToolCallStatus::Running => {}
        }
        self.touch_stats(event);
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ToolUpdated,
                client_thread_id: None,
                agent_id: Some(invocation.agent_id.clone()),
                task_id: Some(invocation.task_id.clone()),
                turn_id: None,
                summary: format!("tool {} {:?}", invocation.tool_name, invocation.status),
            },
        );
    }

    fn apply_approval(&mut self, approval: &Approval, event: &EventEnvelope) {
        if event.event_type == "ApprovalRequested" {
            self.stats.approvals_requested += 1;
        }
        if event.event_type == "ApprovalRecorded" {
            self.stats.approvals_resolved += 1;
        }
        self.touch_stats(event);
        self.approvals.insert(
            approval.approval_id.clone(),
            ApprovalQueueProjection {
                approval_id: approval.approval_id.clone(),
                client_thread_id: None,
                agent_id: approval.requested_by_agent_id.clone(),
                task_id: approval.task_id.clone().unwrap_or_default(),
                status: format!("{:?}", approval.status),
                requested_at: approval.created_at.clone(),
                resolved_at: approval.decided_at.clone(),
                payload: serde_json::to_value(approval).unwrap_or_default(),
            },
        );
        let item_type = if approval.status == ApprovalStatus::Requested {
            TimelineItemType::ApprovalRequested
        } else {
            TimelineItemType::ApprovalResolved
        };
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type,
                client_thread_id: None,
                agent_id: Some(approval.requested_by_agent_id.clone()),
                task_id: approval.task_id.clone(),
                turn_id: None,
                summary: format!("approval {} {:?}", approval.approval_id, approval.status),
            },
        );
    }

    fn apply_artifact(&mut self, artifact: &Artifact, event: &EventEnvelope) {
        self.artifacts.insert(
            artifact.artifact_id.clone(),
            ArtifactIndexProjection {
                artifact_id: artifact.artifact_id.clone(),
                client_thread_id: None,
                agent_id: artifact.owner_agent_id.clone(),
                task_id: artifact.task_id.clone(),
                artifact_type: format!("{:?}", artifact.artifact_type),
                created_at: artifact.created_at.clone(),
                payload: serde_json::to_value(artifact).unwrap_or_default(),
            },
        );
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ArtifactIndexed,
                client_thread_id: None,
                agent_id: Some(artifact.owner_agent_id.clone()),
                task_id: Some(artifact.task_id.clone()),
                turn_id: None,
                summary: format!("artifact {}", artifact.artifact_id),
            },
        );
    }

    fn apply_evidence(&mut self, evidence: &Evidence, event: &EventEnvelope) {
        self.evidence.insert(
            evidence.evidence_id.clone(),
            EvidenceIndexProjection {
                evidence_id: evidence.evidence_id.clone(),
                client_thread_id: None,
                agent_id: evidence.producer_agent_id.clone().unwrap_or_default(),
                task_id: evidence.task_id.clone().unwrap_or_default(),
                evidence_type: format!("{:?}", evidence.evidence_type),
                created_at: evidence.created_at.clone(),
                payload: serde_json::to_value(evidence).unwrap_or_default(),
            },
        );
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::EvidenceIndexed,
                client_thread_id: None,
                agent_id: evidence.producer_agent_id.clone(),
                task_id: evidence.task_id.clone(),
                turn_id: None,
                summary: format!("evidence {}", evidence.evidence_id),
            },
        );
    }

    fn apply_resource_lease(&mut self, lease: &ResourceLease, event: &EventEnvelope) {
        self.resources.insert(
            lease.resource_lease_id.clone(),
            ResourceSessionProjection {
                session_id: lease.resource_lease_id.clone(),
                resource_type: format!("{:?}", lease.resource_type),
                client_thread_id: Some(lease.thread_id.clone()),
                owner_agent_id: Some(lease.owner_agent_id.clone()),
                status: format!("{:?}", lease.status),
                lease_expires_at: lease.lease_expires_at.clone(),
                updated_at: lease
                    .released_at
                    .clone()
                    .unwrap_or_else(|| event.created_at.clone()),
                payload: serde_json::to_value(lease).unwrap_or_default(),
            },
        );
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ResourceUpdated,
                client_thread_id: Some(lease.thread_id.clone()),
                agent_id: Some(lease.owner_agent_id.clone()),
                task_id: Some(lease.task_id.clone()),
                turn_id: None,
                summary: format!("resource lease {}", lease.resource_lease_id),
            },
        );
    }

    fn apply_resource_session(&mut self, session: &ResourceSession, event: &EventEnvelope) {
        self.resources.insert(
            session.session_id.clone(),
            ResourceSessionProjection {
                session_id: session.session_id.clone(),
                resource_type: session.resource_type.as_str().to_string(),
                client_thread_id: session.client_thread_id.clone(),
                owner_agent_id: session.owner_agent_id.clone(),
                status: session.status.as_str().to_string(),
                lease_expires_at: session.lease_expires_at.clone(),
                updated_at: session.updated_at.clone(),
                payload: serde_json::to_value(session).unwrap_or_default(),
            },
        );
        self.push_timeline_item(
            event,
            TimelineDraft {
                item_type: TimelineItemType::ResourceUpdated,
                client_thread_id: session.client_thread_id.clone(),
                agent_id: session.owner_agent_id.clone(),
                task_id: event.task_id.clone(),
                turn_id: None,
                summary: format!("resource session {}", session.session_id),
            },
        );
    }

    fn apply_automation_schedule(&mut self, schedule: &AutomationSchedule) {
        self.automation_schedules
            .insert(schedule.schedule_id.clone(), schedule.clone());
    }

    fn apply_automation_run(&mut self, run: &AutomationRun) {
        self.automation_runs.insert(run.run_id.clone(), run.clone());
    }

    fn touch_stats(&mut self, event: &EventEnvelope) {
        self.stats.updated_at = Some(event.created_at.clone());
    }

    fn push_timeline_item(&mut self, event: &EventEnvelope, draft: TimelineDraft) {
        let item_id = format!("item_{}", event.event_id);
        if self
            .timeline_items
            .iter()
            .any(|item| item.item_id == item_id)
        {
            return;
        }
        self.timeline_items.push(TimelineItem {
            item_id,
            event_id: event.event_id.clone(),
            item_type: draft.item_type,
            client_thread_id: draft.client_thread_id,
            agent_id: draft.agent_id,
            task_id: draft.task_id,
            turn_id: draft.turn_id,
            summary: draft.summary,
            payload: event.payload.clone(),
            created_at: event.created_at.clone(),
        });
    }

    fn update_checkpoints(&mut self, ordinal: u64, updated_at: &str) {
        for name in CHECKPOINT_NAMES {
            self.checkpoints.insert(
                name.to_string(),
                ProjectionCheckpoint {
                    projection_name: name.to_string(),
                    last_event_ordinal: ordinal,
                    updated_at: updated_at.to_string(),
                },
            );
        }
    }

    fn last_projected_ordinal(&self) -> u64 {
        self.checkpoints
            .get("event_stream")
            .map(|checkpoint| checkpoint.last_event_ordinal)
            .unwrap_or_default()
    }
}
