use crate::command_registry::{command_by_slash, CommandTarget};
use crate::{BottomPane, ComposerState, Overlay, TuiExitReport, TuiOptions, TuiProjection};
use agent_os_sys::{AgentOsResult, AppNotificationEnvelope, AppRequest};
use serde_json::Value;

pub trait TuiAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;

    fn read_notification(&mut self) -> AgentOsResult<Option<AppNotificationEnvelope>> {
        Ok(None)
    }
}

impl TuiAppClient for agent_os_host::StdioHostClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        agent_os_host::StdioHostClient::request(self, request)
    }

    fn read_notification(&mut self) -> AgentOsResult<Option<AppNotificationEnvelope>> {
        agent_os_host::StdioHostClient::read_notification(self)
    }
}

pub struct TuiApp<C> {
    client: C,
    pub options: TuiOptions,
    pub projection: TuiProjection,
    pub composer: ComposerState,
    pub bottom_pane: Option<BottomPane>,
    pub overlay: Option<Overlay>,
    pub status_line: String,
    pub raw_mode: bool,
    pub should_exit: bool,
    pub submitted_turns: usize,
    initialized: bool,
}

impl<C: TuiAppClient> TuiApp<C> {
    pub fn new(client: C, options: TuiOptions) -> Self {
        Self {
            client,
            options,
            projection: TuiProjection::default(),
            composer: ComposerState::default(),
            bottom_pane: None,
            overlay: None,
            status_line: "Ready".to_string(),
            raw_mode: false,
            should_exit: false,
            submitted_turns: 0,
            initialized: false,
        }
    }

    pub fn initialize(&mut self) -> AgentOsResult<()> {
        if self.initialized {
            return Ok(());
        }
        self.client.request(AppRequest::Initialize)?;
        self.client
            .request(AppRequest::Subscribe { cursor: None })?;
        if let Some(thread_id) = self.options.thread.clone().or(self.options.resume.clone()) {
            let body = self.client.request(AppRequest::ThreadRead {
                client_thread_id: thread_id,
            })?;
            self.projection.apply_thread_read(&body);
        }
        self.initialized = true;
        Ok(())
    }

    pub fn submit_composer(&mut self) -> AgentOsResult<()> {
        let input = self.composer.take_trimmed();
        self.handle_input(&input)
    }

    pub fn handle_input(&mut self, input: &str) -> AgentOsResult<()> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if let Some(command_text) = trimmed.strip_prefix('/') {
            return self.handle_command(command_text);
        }
        self.submit_user_message(trimmed)
    }

    pub fn handle_command(&mut self, command_text: &str) -> AgentOsResult<()> {
        let Some(command) = command_by_slash(command_text) else {
            self.status_line = format!("Unknown command /{command_text}");
            return Ok(());
        };
        let args = command_text
            .find(char::is_whitespace)
            .map(|index| command_text[index..].trim())
            .unwrap_or("");
        match command.slash {
            "exit" => {
                self.should_exit = true;
                self.status_line = "Exit requested".to_string();
            }
            "new" => {
                self.projection = TuiProjection::default();
                self.status_line = "New thread composer".to_string();
            }
            "run" | "plan" => {
                let prompt = if args.is_empty() {
                    self.composer.text.trim().to_string()
                } else {
                    args.to_string()
                };
                self.submit_user_message(&prompt)?;
            }
            "steer" => {
                if let Some(turn_id) = self.projection.current_turn_id.clone() {
                    let body = self.client.request(AppRequest::TurnSteer {
                        turn_id,
                        input: args.to_string(),
                    })?;
                    self.projection.current_turn_id =
                        body["turn"]["turn_id"].as_str().map(str::to_string);
                    self.status_line = "Steering input sent".to_string();
                } else {
                    self.status_line = "No active turn to steer".to_string();
                }
            }
            "interrupt" => {
                if let Some(turn_id) = self.projection.current_turn_id.clone() {
                    let body = self.client.request(AppRequest::TurnInterrupt { turn_id })?;
                    self.projection.apply_turn_start(&body);
                    self.status_line = "Turn interrupted".to_string();
                } else {
                    self.status_line = "No active turn to interrupt".to_string();
                }
            }
            "threads" => {
                let body = self
                    .client
                    .request(AppRequest::ThreadList { archived: None })?;
                self.bottom_pane = Some(BottomPane::Threads);
                self.status_line = format!(
                    "{} thread(s)",
                    body["threads"].as_array().map(Vec::len).unwrap_or_default()
                );
            }
            "resume" => {
                let body = self.client.request(AppRequest::ThreadResume {
                    client_thread_id: args.to_string(),
                })?;
                self.projection.current_thread_id = body["thread"]["client_thread_id"]
                    .as_str()
                    .map(str::to_string);
                self.status_line = "Thread resumed".to_string();
            }
            "fork" => {
                let body = self.client.request(AppRequest::ThreadFork {
                    client_thread_id: self.require_thread_id()?,
                    from_turn_id: (!args.is_empty()).then(|| args.to_string()),
                    title: None,
                    goal: None,
                })?;
                self.projection.current_thread_id = body["thread"]["client_thread_id"]
                    .as_str()
                    .map(str::to_string);
                self.status_line = "Thread forked".to_string();
            }
            "rollback" => {
                self.client.request(AppRequest::ThreadRollback {
                    client_thread_id: self.require_thread_id()?,
                    target_turn_id: (!args.is_empty()).then(|| args.to_string()),
                    target_item_id: None,
                    target_event_id: None,
                    reason: "rollback requested from TUI".to_string(),
                })?;
                self.status_line = "Rollback requested".to_string();
            }
            "rename" => {
                self.client.request(AppRequest::ThreadNameSet {
                    client_thread_id: self.require_thread_id()?,
                    title: args.to_string(),
                })?;
                self.status_line = "Thread renamed".to_string();
            }
            "archive" => {
                self.client.request(AppRequest::ThreadArchive {
                    client_thread_id: self.require_thread_id()?,
                })?;
                self.status_line = "Thread archived".to_string();
            }
            "delete" => {
                self.client.request(AppRequest::ThreadDelete {
                    client_thread_id: self.require_thread_id()?,
                })?;
                self.status_line = "Thread deleted".to_string();
            }
            "compact" => {
                self.client.request(AppRequest::ThreadCompact {
                    client_thread_id: self.require_thread_id()?,
                    summary_artifact_id: None,
                    superseded_refs: Vec::new(),
                    token_estimate: 0,
                })?;
                self.status_line = "Context compaction requested".to_string();
            }
            "raw" => {
                self.raw_mode = args != "off";
                self.status_line = format!("Raw mode {}", if self.raw_mode { "on" } else { "off" });
            }
            "clear" => {
                self.projection.timeline.clear();
                self.status_line = "Scrollback cleared".to_string();
            }
            "status" => {
                self.bottom_pane = Some(BottomPane::Status);
                self.status_line = "Opened status panel".to_string();
            }
            "model" => {
                if args.is_empty() {
                    self.bottom_pane = Some(BottomPane::Models);
                    self.status_line = "Opened model picker".to_string();
                } else {
                    self.options.model = Some(args.to_string());
                    self.status_line = format!("Model set to {args}");
                }
            }
            "profile" => {
                if args.is_empty() {
                    self.bottom_pane = Some(BottomPane::Models);
                    self.status_line = "Opened profile panel".to_string();
                } else {
                    self.options.profile = Some(args.to_string());
                    self.status_line = format!("Profile set to {args}");
                }
            }
            "permissions" => {
                self.bottom_pane = Some(BottomPane::Permissions);
                self.status_line = "Opened permissions panel".to_string();
            }
            "approve" => {
                if args.is_empty() {
                    self.bottom_pane = Some(BottomPane::Approvals);
                    self.status_line = "Opened approvals panel".to_string();
                } else {
                    let (approval_id, approved) = parse_approval_response(args)?;
                    self.client.request(AppRequest::ApprovalRespond {
                        approval_id: approval_id.to_string(),
                        approved,
                    })?;
                    self.status_line = if approved {
                        format!("Approved {approval_id}")
                    } else {
                        format!("Denied {approval_id}")
                    };
                }
            }
            "processes" => {
                self.bottom_pane = Some(BottomPane::Processes);
                self.status_line = "Opened process panel".to_string();
            }
            "stop" => {
                self.client.request(AppRequest::ProcessStop {
                    process_id: args.to_string(),
                    reason: Some("stopped from TUI".to_string()),
                })?;
                self.status_line = "Process stop requested".to_string();
            }
            "kill" => {
                self.client.request(AppRequest::ProcessKill {
                    process_id: args.to_string(),
                    reason: Some("killed from TUI".to_string()),
                })?;
                self.status_line = "Process kill requested".to_string();
            }
            "help" => {
                self.overlay = Some(Overlay::Help);
                self.status_line = "Opened help".to_string();
            }
            "keymap" => {
                self.overlay = Some(Overlay::Keymap);
                self.status_line = "Opened keymap".to_string();
            }
            "tools" => {
                self.overlay = Some(Overlay::Tools);
                self.status_line = "Opened tools".to_string();
            }
            "mcp" => {
                self.overlay = Some(Overlay::Mcp);
                self.status_line = "Opened MCP".to_string();
            }
            "provider" => {
                self.overlay = Some(Overlay::Provider);
                self.status_line = "Opened provider".to_string();
            }
            "usage" => {
                self.overlay = Some(Overlay::Usage);
                self.status_line = "Opened usage".to_string();
            }
            "context" => {
                self.overlay = Some(Overlay::Context);
                self.status_line = "Opened context".to_string();
            }
            "events" => {
                self.overlay = Some(Overlay::Events);
                self.status_line = "Opened events".to_string();
            }
            "replay" => {
                self.overlay = Some(Overlay::Replay);
                self.status_line = "Opened replay".to_string();
            }
            "evidence" => {
                self.overlay = Some(Overlay::Evidence);
                self.status_line = "Opened evidence".to_string();
            }
            "artifacts" => {
                self.overlay = Some(Overlay::Artifacts);
                self.status_line = "Opened artifacts".to_string();
            }
            "diff" => {
                self.overlay = Some(Overlay::Diff);
                self.status_line = "Opened diff".to_string();
            }
            "debug" => {
                self.overlay = Some(Overlay::Debug);
                self.status_line = "Opened debug".to_string();
            }
            _ if command.target == CommandTarget::ProjectionOnly
                || command.target == CommandTarget::UiOnly =>
            {
                self.status_line = format!("Opened /{} panel", command.slash);
            }
            _ => self.status_line = format!("/{} is not wired yet", command.slash),
        }
        Ok(())
    }

    pub fn refresh_current_thread(&mut self) -> AgentOsResult<()> {
        let Some(thread_id) = self.projection.current_thread_id.clone() else {
            return Ok(());
        };
        let body = self.client.request(AppRequest::ThreadRead {
            client_thread_id: thread_id,
        })?;
        self.projection.apply_thread_read(&body);
        Ok(())
    }

    pub fn close_top_mode(&mut self) {
        if self.overlay.take().is_none() {
            self.bottom_pane = None;
        }
    }

    pub fn drain_notifications(&mut self) -> AgentOsResult<()> {
        while let Some(notification) = self.client.read_notification()? {
            self.projection.apply_notification(&notification);
        }
        Ok(())
    }

    pub fn exit_report(&self) -> TuiExitReport {
        TuiExitReport {
            last_thread_id: self.projection.current_thread_id.clone(),
            submitted_turns: self.submitted_turns,
            final_status: self.projection.thread_status.clone(),
        }
    }

    fn submit_user_message(&mut self, input: &str) -> AgentOsResult<()> {
        if input.trim().is_empty() {
            self.status_line = "No prompt to submit".to_string();
            return Ok(());
        }
        if self.projection.running() {
            let Some(turn_id) = self.projection.current_turn_id.clone() else {
                self.status_line = "Running thread has no turn id".to_string();
                return Ok(());
            };
            let body = self.client.request(AppRequest::TurnSteer {
                turn_id,
                input: input.to_string(),
            })?;
            self.projection.current_turn_id = body["turn"]["turn_id"].as_str().map(str::to_string);
            self.status_line = "Steering input sent".to_string();
            return Ok(());
        }
        let thread_id = match self.projection.current_thread_id.clone() {
            Some(thread_id) => thread_id,
            None => {
                let body = self.client.request(AppRequest::ThreadStart {
                    goal: input.to_string(),
                    workspace: self
                        .options
                        .workspace
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string()),
                })?;
                body["thread"]["client_thread_id"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_default()
            }
        };
        let body = self.client.request(AppRequest::TurnStart {
            client_thread_id: thread_id,
            input: input.to_string(),
        })?;
        self.projection.apply_turn_start(&body);
        self.submitted_turns += 1;
        self.status_line = "Prompt submitted".to_string();
        Ok(())
    }

    fn require_thread_id(&self) -> AgentOsResult<String> {
        self.projection.current_thread_id.clone().ok_or_else(|| {
            agent_os_sys::AgentOsError::Validation("no current thread selected".to_string())
        })
    }
}

fn parse_approval_response(args: &str) -> AgentOsResult<(&str, bool)> {
    let mut parts = args.split_whitespace();
    let approval_id = parts.next().ok_or_else(|| {
        agent_os_sys::AgentOsError::Validation(
            "/approve requires <approval-id> approve|deny".to_string(),
        )
    })?;
    let decision = parts.next().ok_or_else(|| {
        agent_os_sys::AgentOsError::Validation(
            "/approve requires <approval-id> approve|deny".to_string(),
        )
    })?;
    let approved = match decision {
        "approve" | "approved" | "yes" | "y" => true,
        "deny" | "denied" | "no" | "n" => false,
        other => {
            return Err(agent_os_sys::AgentOsError::Validation(format!(
                "unknown approval decision {other}; use approve or deny"
            )))
        }
    };
    Ok((approval_id, approved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_message_starts_thread_and_turn() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());

        app.handle_input("Fix the tests").unwrap();

        assert_eq!(
            app.projection.current_thread_id.as_deref(),
            Some("thread_1")
        );
        assert_eq!(app.projection.current_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(app.submitted_turns, 1);
        assert_eq!(app.client.requests, vec!["thread/start", "turn/start"]);
    }

    #[test]
    fn interrupt_maps_to_app_request() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());
        app.projection.current_turn_id = Some("turn_1".to_string());

        app.handle_input("/interrupt").unwrap();

        assert_eq!(app.status_line, "Turn interrupted");
        assert_eq!(app.client.requests, vec!["turn/interrupt"]);
    }

    #[test]
    fn plain_message_during_running_turn_steers_turn() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());
        app.projection.current_turn_id = Some("turn_1".to_string());
        app.projection.thread_status = Some("Running".to_string());

        app.handle_input("narrow the fix").unwrap();

        assert_eq!(app.status_line, "Steering input sent");
        assert_eq!(app.client.requests, vec!["turn/steer"]);
    }

    #[test]
    fn thread_lifecycle_commands_map_to_app_requests() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());
        app.projection.current_thread_id = Some("thread_1".to_string());

        app.handle_input("/fork turn_1").unwrap();
        app.handle_input("/rollback turn_1").unwrap();
        app.handle_input("/compact").unwrap();

        assert_eq!(
            app.client.requests,
            vec!["thread/fork", "thread/rollback", "thread/compact"]
        );
    }

    #[test]
    fn projection_only_commands_open_overlay_without_request() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());

        app.handle_input("/tools").unwrap();

        assert_eq!(app.overlay, Some(Overlay::Tools));
        assert!(app.client.requests.is_empty());
    }

    #[test]
    fn approve_with_decision_maps_to_approval_respond() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());

        app.handle_input("/approve approval_1 approve").unwrap();

        assert_eq!(app.status_line, "Approved approval_1");
        assert_eq!(app.client.requests, vec!["approval/respond"]);
    }

    #[test]
    fn initialize_without_explicit_thread_does_not_subscribe_or_select_history() {
        let mut app = TuiApp::new(FakeTuiClient::default(), TuiOptions::default());

        app.initialize().unwrap();

        assert_eq!(app.projection.current_thread_id, None);
        assert_eq!(app.client.requests, vec!["initialize", "subscribe"]);
    }

    #[derive(Default)]
    struct FakeTuiClient {
        requests: Vec<&'static str>,
    }

    impl TuiAppClient for FakeTuiClient {
        fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
            match request {
                AppRequest::Initialize => {
                    self.requests.push("initialize");
                    Ok(json!({"initialized": true}))
                }
                AppRequest::Subscribe { cursor } => {
                    self.requests.push("subscribe");
                    assert_eq!(cursor, None);
                    Ok(json!({
                        "subscription_id": "sub_1",
                        "cursor": {"last_event_ordinal": 12}
                    }))
                }
                AppRequest::ThreadStart { goal, .. } => {
                    self.requests.push("thread/start");
                    assert_eq!(goal, "Fix the tests");
                    Ok(json!({"thread": {"client_thread_id": "thread_1", "status": "Ready"}}))
                }
                AppRequest::TurnStart {
                    client_thread_id,
                    input,
                } => {
                    self.requests.push("turn/start");
                    assert_eq!(client_thread_id, "thread_1");
                    assert_eq!(input, "Fix the tests");
                    Ok(json!({
                        "thread": {"client_thread_id": "thread_1", "status": "Running"},
                        "turn": {"turn_id": "turn_1"},
                        "runtime_job": {"runtime_job_id": "rtjob_1", "status": "queued"}
                    }))
                }
                AppRequest::TurnInterrupt { turn_id } => {
                    self.requests.push("turn/interrupt");
                    assert_eq!(turn_id, "turn_1");
                    Ok(json!({
                        "thread": {"client_thread_id": "thread_1", "status": "Interrupted"},
                        "turn": {"turn_id": "turn_1"},
                        "runtime_jobs": []
                    }))
                }
                AppRequest::TurnSteer { turn_id, input } => {
                    self.requests.push("turn/steer");
                    assert_eq!(turn_id, "turn_1");
                    assert_eq!(input, "narrow the fix");
                    Ok(json!({"turn": {"turn_id": "turn_1"}}))
                }
                AppRequest::ThreadFork {
                    client_thread_id,
                    from_turn_id,
                    ..
                } => {
                    self.requests.push("thread/fork");
                    assert_eq!(client_thread_id, "thread_1");
                    assert_eq!(from_turn_id.as_deref(), Some("turn_1"));
                    Ok(json!({"thread": {"client_thread_id": "thread_fork"}}))
                }
                AppRequest::ThreadRollback {
                    client_thread_id,
                    target_turn_id,
                    ..
                } => {
                    self.requests.push("thread/rollback");
                    assert_eq!(client_thread_id, "thread_fork");
                    assert_eq!(target_turn_id.as_deref(), Some("turn_1"));
                    Ok(json!({}))
                }
                AppRequest::ThreadCompact {
                    client_thread_id, ..
                } => {
                    self.requests.push("thread/compact");
                    assert_eq!(client_thread_id, "thread_fork");
                    Ok(json!({}))
                }
                AppRequest::ApprovalRespond {
                    approval_id,
                    approved,
                } => {
                    self.requests.push("approval/respond");
                    assert_eq!(approval_id, "approval_1");
                    assert!(approved);
                    Ok(json!({"approval": {"approval_id": "approval_1", "status": "approved"}}))
                }
                other => panic!("unexpected request {other:?}"),
            }
        }
    }
}
