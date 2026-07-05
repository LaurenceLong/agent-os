use agent_os_sys::{AppNotification, AppNotificationEnvelope, TimelineItemType};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct TuiProjection {
    pub current_thread_id: Option<String>,
    pub current_turn_id: Option<String>,
    pub thread_status: Option<String>,
    pub timeline: Vec<String>,
    pub runtime_jobs: Vec<Value>,
    pub process_sessions: Vec<Value>,
    pub artifacts: Vec<Value>,
    pub evidence: Vec<Value>,
    pub resources: Vec<Value>,
    pub automation_runs: Vec<Value>,
    pub raw: Option<Value>,
}

impl TuiProjection {
    pub fn apply_thread_read(&mut self, body: &Value) {
        self.raw = Some(body.clone());
        self.current_thread_id = body["thread"]["client_thread_id"]
            .as_str()
            .map(str::to_string)
            .or_else(|| self.current_thread_id.clone());
        self.thread_status = body["thread"]["status"].as_str().map(str::to_string);
        self.current_turn_id = body["turns"]
            .as_array()
            .and_then(|turns| turns.last())
            .and_then(|turn| turn["turn_id"].as_str())
            .map(str::to_string)
            .or_else(|| self.current_turn_id.clone());
        self.timeline = body["timeline"]
            .as_array()
            .map(|items| items.iter().map(format_timeline_item).collect())
            .unwrap_or_default();
        self.runtime_jobs = body["runtime_jobs"].as_array().cloned().unwrap_or_default();
        self.process_sessions = body["process_sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        self.artifacts = body["artifacts"].as_array().cloned().unwrap_or_default();
        self.evidence = body["evidence"].as_array().cloned().unwrap_or_default();
        self.resources = body["resources"].as_array().cloned().unwrap_or_default();
        self.automation_runs = body["automation_runs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
    }

    pub fn apply_turn_start(&mut self, body: &Value) {
        self.current_thread_id = body["thread"]["client_thread_id"]
            .as_str()
            .map(str::to_string);
        self.current_turn_id = body["turn"]["turn_id"].as_str().map(str::to_string);
        self.thread_status = body["thread"]["status"].as_str().map(str::to_string);
        if body["runtime_job"].is_object() {
            self.runtime_jobs.push(body["runtime_job"].clone());
        }
    }

    pub fn apply_notification(&mut self, envelope: &AppNotificationEnvelope) {
        match &envelope.notification {
            AppNotification::ThreadChanged(thread) => {
                self.current_thread_id = Some(thread.client_thread_id.clone());
                self.thread_status = Some(format!("{:?}", thread.status));
            }
            AppNotification::TurnStarted(turn) | AppNotification::TurnCompleted(turn) => {
                self.current_turn_id = Some(turn.turn_id.clone());
                self.timeline
                    .push(format!("turn {} {:?}", turn.turn_id, turn.status));
            }
            AppNotification::ItemStarted(item)
            | AppNotification::ItemCompleted(item)
            | AppNotification::AgentMessageDelta(item)
            | AppNotification::ToolUpdate(item) => {
                self.timeline.push(format_timeline_payload(
                    timeline_item_type_label(item.item_type),
                    &item.payload,
                ));
            }
            AppNotification::ApprovalRequested(approval)
            | AppNotification::ApprovalResolved(approval) => {
                self.timeline.push(format!(
                    "approval {} {:?}",
                    approval.approval_id, approval.status
                ));
            }
            AppNotification::ArtifactIndexed(artifact) => {
                self.timeline
                    .push(format!("artifact {}", artifact.artifact_id));
            }
            AppNotification::EvidenceIndexed(evidence) => {
                self.timeline
                    .push(format!("evidence {}", evidence.evidence_id));
            }
            AppNotification::ResourceUpdated(resource) => {
                self.timeline
                    .push(format!("resource {}", resource.session_id));
            }
            AppNotification::StatsUpdated(_) => {}
        }
    }

    pub fn running(&self) -> bool {
        matches!(
            self.thread_status.as_deref(),
            Some("Running" | "WaitingTool" | "Completing")
        )
    }
}

fn format_timeline_item(item: &Value) -> String {
    let item_type = item["item_type"].as_str().unwrap_or("item");
    format_timeline_payload(item_type, &item["payload"])
}

fn format_timeline_payload(item_type: &str, payload: &Value) -> String {
    match item_type {
        "ToolUpdated" => format!(
            "tool {} {}",
            payload["tool_name"].as_str().unwrap_or("?"),
            payload["status"].as_str().unwrap_or("?")
        ),
        "AgentMessageDelta" => payload["text"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "agent message".to_string()),
        _ => format!("{item_type}: {payload}"),
    }
}

fn timeline_item_type_label(item_type: TimelineItemType) -> &'static str {
    match item_type {
        TimelineItemType::ThreadChanged => "ThreadChanged",
        TimelineItemType::TurnStarted => "TurnStarted",
        TimelineItemType::TurnCompleted => "TurnCompleted",
        TimelineItemType::ItemStarted => "ItemStarted",
        TimelineItemType::ItemCompleted => "ItemCompleted",
        TimelineItemType::AgentMessageDelta => "AgentMessageDelta",
        TimelineItemType::ToolUpdated => "ToolUpdated",
        TimelineItemType::ApprovalRequested => "ApprovalRequested",
        TimelineItemType::ApprovalResolved => "ApprovalResolved",
        TimelineItemType::StatsUpdated => "StatsUpdated",
        TimelineItemType::ArtifactIndexed => "ArtifactIndexed",
        TimelineItemType::EvidenceIndexed => "EvidenceIndexed",
        TimelineItemType::ResourceUpdated => "ResourceUpdated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projection_applies_thread_read() {
        let mut projection = TuiProjection::default();

        projection.apply_thread_read(&json!({
            "thread": {"client_thread_id": "thread_1", "status": "Ready"},
            "turns": [{"turn_id": "turn_1"}],
            "timeline": [{"item_type": "ToolUpdated", "payload": {"tool_name": "read_file", "status": "Completed"}}],
            "runtime_jobs": [],
            "artifacts": [],
            "evidence": [],
            "resources": [],
            "automation_runs": []
        }));

        assert_eq!(projection.current_thread_id.as_deref(), Some("thread_1"));
        assert_eq!(projection.current_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(projection.timeline, vec!["tool read_file Completed"]);
    }
}
