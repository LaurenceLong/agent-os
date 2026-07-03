use crate::AgentOsHost;
use agent_os_sys::{
    app_protocol_version, AgentOsResult, AppNotification, AppNotificationEnvelope,
    ApprovalQueueProjection, ArtifactIndexProjection, ClientThread, EventEnvelope,
    EvidenceIndexProjection, ProjectionCursor, ResourceSessionProjection, StatsQuery,
    StatsSnapshot, TimelineItem, TimelineItemType, TurnRecord,
};
use std::collections::BTreeMap;

struct ProjectionIndex {
    threads: BTreeMap<String, ClientThread>,
    turns: BTreeMap<String, TurnRecord>,
    timeline_by_event: BTreeMap<String, TimelineItem>,
    approvals: BTreeMap<String, ApprovalQueueProjection>,
    resources: BTreeMap<String, ResourceSessionProjection>,
    artifacts: BTreeMap<String, ArtifactIndexProjection>,
    evidence: BTreeMap<String, EvidenceIndexProjection>,
    stats: StatsSnapshot,
}

impl ProjectionIndex {
    fn load(host: &AgentOsHost) -> AgentOsResult<Self> {
        let store = host.kernel().store();
        Ok(Self {
            threads: store
                .thread_summaries()?
                .into_iter()
                .map(|thread| (thread.client_thread_id.clone(), thread))
                .collect(),
            turns: store
                .turn_summaries()?
                .into_iter()
                .map(|turn| (turn.turn_id.clone(), turn))
                .collect(),
            timeline_by_event: store
                .timeline_items(None)?
                .into_iter()
                .map(|item| (item.event_id.clone(), item))
                .collect(),
            approvals: store
                .approval_queue()?
                .into_iter()
                .map(|approval| (approval.approval_id.clone(), approval))
                .collect(),
            resources: store
                .resource_sessions()?
                .into_iter()
                .map(|resource| (resource.session_id.clone(), resource))
                .collect(),
            artifacts: store
                .artifact_index()?
                .into_iter()
                .map(|artifact| (artifact.artifact_id.clone(), artifact))
                .collect(),
            evidence: store
                .evidence_index()?
                .into_iter()
                .map(|evidence| (evidence.evidence_id.clone(), evidence))
                .collect(),
            stats: store.stats_snapshot(StatsQuery::default())?,
        })
    }
}

impl AgentOsHost {
    pub fn notifications_since(
        &self,
        cursor: &ProjectionCursor,
    ) -> AgentOsResult<Vec<AppNotificationEnvelope>> {
        let events = self.kernel().events()?;
        let index = ProjectionIndex::load(self)?;
        let mut notifications = Vec::new();
        for (event_index, event) in events.iter().enumerate() {
            let ordinal = event_index as u64 + 1;
            if ordinal <= cursor.last_event_ordinal {
                continue;
            }
            for notification in notifications_for_event(event, &index) {
                notifications.push(AppNotificationEnvelope {
                    protocol: app_protocol_version(),
                    subscription_id: None,
                    cursor: ProjectionCursor {
                        last_event_ordinal: ordinal,
                    },
                    notification,
                });
            }
        }
        Ok(notifications)
    }
}

fn notifications_for_event(event: &EventEnvelope, index: &ProjectionIndex) -> Vec<AppNotification> {
    let mut notifications = Vec::new();
    if event_updates_stats(event) {
        notifications.push(AppNotification::StatsUpdated(index.stats.clone()));
    }
    let Some(item) = index.timeline_by_event.get(&event.event_id) else {
        return notifications;
    };
    match item.item_type {
        TimelineItemType::ThreadChanged => {
            if let Some(thread_id) = &item.client_thread_id {
                if let Some(thread) = index.threads.get(thread_id) {
                    notifications.push(AppNotification::ThreadChanged(thread.clone()));
                }
            }
            if event.event_type == "ThreadStatusChanged" {
                push_completed_turn(item, index, &mut notifications);
            }
        }
        TimelineItemType::TurnStarted => {
            if let Some(turn_id) = &item.turn_id {
                if let Some(turn) = index.turns.get(turn_id) {
                    notifications.push(AppNotification::TurnStarted(turn.clone()));
                }
            }
        }
        TimelineItemType::TurnCompleted => {
            if let Some(turn_id) = &item.turn_id {
                if let Some(turn) = index.turns.get(turn_id) {
                    notifications.push(AppNotification::TurnCompleted(turn.clone()));
                }
            }
        }
        TimelineItemType::ItemStarted => {
            notifications.push(AppNotification::ItemStarted(item.clone()));
        }
        TimelineItemType::ItemCompleted => {
            notifications.push(AppNotification::ItemCompleted(item.clone()));
        }
        TimelineItemType::AgentMessageDelta => {
            notifications.push(AppNotification::AgentMessageDelta(item.clone()));
        }
        TimelineItemType::ToolUpdated => {
            notifications.push(AppNotification::ToolUpdate(item.clone()));
        }
        TimelineItemType::ApprovalRequested => {
            if let Some(approval) = item
                .payload
                .get("approval_id")
                .and_then(|value| value.as_str())
                .and_then(|approval_id| index.approvals.get(approval_id))
            {
                notifications.push(AppNotification::ApprovalRequested(approval.clone()));
            }
        }
        TimelineItemType::ApprovalResolved => {
            if let Some(approval) = item
                .payload
                .get("approval_id")
                .and_then(|value| value.as_str())
                .and_then(|approval_id| index.approvals.get(approval_id))
            {
                notifications.push(AppNotification::ApprovalResolved(approval.clone()));
            }
        }
        TimelineItemType::StatsUpdated => {
            if !event_updates_stats(event) {
                notifications.push(AppNotification::StatsUpdated(index.stats.clone()));
            }
        }
        TimelineItemType::ArtifactIndexed => {
            if let Some(artifact) = item
                .payload
                .get("artifact_id")
                .and_then(|value| value.as_str())
                .and_then(|artifact_id| index.artifacts.get(artifact_id))
            {
                notifications.push(AppNotification::ArtifactIndexed(artifact.clone()));
            }
        }
        TimelineItemType::EvidenceIndexed => {
            if let Some(evidence) = item
                .payload
                .get("evidence_id")
                .and_then(|value| value.as_str())
                .and_then(|evidence_id| index.evidence.get(evidence_id))
            {
                notifications.push(AppNotification::EvidenceIndexed(evidence.clone()));
            }
        }
        TimelineItemType::ResourceUpdated => {
            if let Some(resource) = item
                .payload
                .get("resource_lease_id")
                .or_else(|| item.payload.get("session_id"))
                .and_then(|value| value.as_str())
                .and_then(|resource_id| index.resources.get(resource_id))
            {
                notifications.push(AppNotification::ResourceUpdated(resource.clone()));
            }
        }
    }
    notifications
}

fn push_completed_turn(
    item: &TimelineItem,
    index: &ProjectionIndex,
    notifications: &mut Vec<AppNotification>,
) {
    let Some(turn_id) = &item.turn_id else {
        return;
    };
    let Some(turn) = index.turns.get(turn_id) else {
        return;
    };
    if turn.completed_at.is_some() {
        notifications.push(AppNotification::TurnCompleted(turn.clone()));
    }
}

fn event_updates_stats(event: &EventEnvelope) -> bool {
    matches!(
        event.event_type.as_str(),
        "ProviderUsageRecorded"
            | "ProviderStreamFailed"
            | "ProviderStreamCancelled"
            | "ToolCallProgressed"
            | "ToolCallCompleted"
            | "ToolCallFailed"
            | "ToolCallDenied"
            | "ToolCallReconciled"
            | "ApprovalRequested"
            | "ApprovalRecorded"
            | "BudgetDebited"
    )
}
