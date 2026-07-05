//! Stdio JSONL app-server protocol for Agent-OS applications.
//!
//! The app-server is deliberately a transport and protocol gate. Kernel state
//! changes are delegated to the host service behind `AppKernelService`.

use agent_os_sys::{
    app_protocol_version, AgentOsError, AgentOsResult, AppNotificationEnvelope, AppRequest,
    AppRequestEnvelope, AppResponse, AppResponseEnvelope, ClientConnection, ClientKind,
    ProjectionCursor, SecurityLevel,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};

pub trait AppKernelService {
    fn handle_app_request(
        &self,
        client: &ClientConnection,
        request: AppRequest,
    ) -> AgentOsResult<AppResponse>;

    fn app_notifications_since(
        &self,
        _cursor: &ProjectionCursor,
    ) -> AgentOsResult<Vec<AppNotificationEnvelope>> {
        Ok(Vec::new())
    }

    fn current_projection_cursor(&self) -> AgentOsResult<ProjectionCursor> {
        Ok(ProjectionCursor {
            last_event_ordinal: 0,
        })
    }
}

pub struct ThreadReadProjection<
    TThread,
    TTurns,
    TTimeline,
    TJobs,
    TProcessSessions,
    TArtifacts,
    TEvidence,
    TResources,
    TAutomationRuns,
> {
    pub thread: TThread,
    pub turns: TTurns,
    pub timeline: TTimeline,
    pub runtime_jobs: TJobs,
    pub process_sessions: TProcessSessions,
    pub artifacts: TArtifacts,
    pub evidence: TEvidence,
    pub resources: TResources,
    pub automation_runs: TAutomationRuns,
}

pub fn thread_read_response<
    TThread,
    TTurns,
    TTimeline,
    TJobs,
    TProcessSessions,
    TArtifacts,
    TEvidence,
    TResources,
    TAutomationRuns,
>(
    projection: ThreadReadProjection<
        TThread,
        TTurns,
        TTimeline,
        TJobs,
        TProcessSessions,
        TArtifacts,
        TEvidence,
        TResources,
        TAutomationRuns,
    >,
) -> AgentOsResult<AppResponse>
where
    TThread: Serialize,
    TTurns: Serialize,
    TTimeline: Serialize,
    TJobs: Serialize,
    TProcessSessions: Serialize,
    TArtifacts: Serialize,
    TEvidence: Serialize,
    TResources: Serialize,
    TAutomationRuns: Serialize,
{
    Ok(AppResponse::Accepted(json!({
        "thread": projection.thread,
        "turns": projection.turns,
        "timeline": projection.timeline,
        "runtime_jobs": projection.runtime_jobs,
        "process_sessions": projection.process_sessions,
        "artifacts": projection.artifacts,
        "evidence": projection.evidence,
        "resources": projection.resources,
        "automation_runs": projection.automation_runs,
    })))
}

pub struct JsonlAppClient<R, W> {
    client: ClientConnection,
    reader: R,
    writer: W,
    next_request_id: u64,
    pending_notifications: VecDeque<AppNotificationEnvelope>,
}

impl<R, W> JsonlAppClient<R, W>
where
    R: BufRead,
    W: Write,
{
    pub fn new(client: ClientConnection, reader: R, writer: W) -> Self {
        Self {
            client,
            reader,
            writer,
            next_request_id: 1,
            pending_notifications: VecDeque::new(),
        }
    }

    pub fn request(&mut self, request: AppRequest) -> AgentOsResult<AppResponseEnvelope> {
        let request_id = format!("req_{:016x}", self.next_request_id);
        self.next_request_id += 1;
        self.request_with_id(request_id, request)
    }

    pub fn request_with_id(
        &mut self,
        request_id: impl Into<String>,
        request: AppRequest,
    ) -> AgentOsResult<AppResponseEnvelope> {
        let envelope = AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: request_id.into(),
            client: self.client.clone(),
            request,
        };
        let line = serde_json::to_string(&envelope)?;
        writeln!(self.writer, "{line}")
            .map_err(|error| AgentOsError::Validation(format!("write app request: {error}")))?;
        self.writer
            .flush()
            .map_err(|error| AgentOsError::Validation(format!("flush app request: {error}")))?;
        loop {
            let mut line = String::new();
            let bytes = self
                .reader
                .read_line(&mut line)
                .map_err(|error| AgentOsError::Validation(format!("read app response: {error}")))?;
            if bytes == 0 {
                return Err(AgentOsError::Validation(
                    "app-server closed before response".to_string(),
                ));
            }
            let line = line.trim_end();
            if let Ok(response) = serde_json::from_str::<AppResponseEnvelope>(line) {
                return Ok(response);
            }
            let notification: AppNotificationEnvelope = serde_json::from_str(line)?;
            self.pending_notifications.push_back(notification);
        }
    }

    pub fn read_notification(&mut self) -> AgentOsResult<Option<AppNotificationEnvelope>> {
        if let Some(notification) = self.pending_notifications.pop_front() {
            return Ok(Some(notification));
        }
        let mut line = String::new();
        let bytes = self
            .reader
            .read_line(&mut line)
            .map_err(|error| AgentOsError::Validation(format!("read app notification: {error}")))?;
        if bytes == 0 {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(line.trim_end())?))
    }

    pub fn take_pending_notification(&mut self) -> Option<AppNotificationEnvelope> {
        self.pending_notifications.pop_front()
    }

    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }
}

pub struct AppServer<S> {
    service: S,
    initialized_clients: HashSet<String>,
    subscriptions: HashMap<String, ProjectionCursor>,
    next_subscription_id: u64,
}

impl<S> AppServer<S>
where
    S: AppKernelService,
{
    pub fn new(service: S) -> Self {
        Self {
            service,
            initialized_clients: HashSet::new(),
            subscriptions: HashMap::new(),
            next_subscription_id: 1,
        }
    }

    pub fn handle_line(&mut self, line: &str) -> String {
        let response = match serde_json::from_str::<AppRequestEnvelope>(line) {
            Ok(envelope) => self.handle_envelope(envelope),
            Err(error) => AppResponseEnvelope {
                protocol: app_protocol_version(),
                request_id: String::new(),
                response: reject("invalid_json", error.to_string()),
            },
        };
        serde_json::to_string(&response)
            .unwrap_or_else(|error| fallback_serialization_error(error.to_string()))
    }

    pub fn handle_envelope(&mut self, envelope: AppRequestEnvelope) -> AppResponseEnvelope {
        let request_id = envelope.request_id.clone();
        let response = if envelope.protocol != agent_os_sys::APP_PROTOCOL_VERSION {
            reject(
                "unsupported_protocol",
                format!("expected protocol {}", agent_os_sys::APP_PROTOCOL_VERSION),
            )
        } else {
            match envelope.request {
                AppRequest::Initialize => self.initialize(envelope.client),
                AppRequest::Subscribe { cursor } => self
                    .with_initialized_client(&envelope.client, |server| server.subscribe(cursor)),
                AppRequest::Unsubscribe { subscription_id } => self
                    .with_initialized_client(&envelope.client, |server| {
                        server.unsubscribe(subscription_id)
                    }),
                request => self.with_initialized_client(&envelope.client, |server| {
                    match server.service.handle_app_request(&envelope.client, request) {
                        Ok(response) => response,
                        Err(error) => error_response(error),
                    }
                }),
            }
        };
        AppResponseEnvelope {
            protocol: app_protocol_version(),
            request_id,
            response,
        }
    }

    pub fn serve_jsonl<R, W>(&mut self, reader: R, mut writer: W) -> AgentOsResult<()>
    where
        R: BufRead,
        W: Write,
    {
        for line in reader.lines() {
            let line = line.map_err(|error| AgentOsError::Validation(error.to_string()))?;
            let response = self.handle_line(&line);
            writeln!(writer, "{response}")
                .map_err(|error| AgentOsError::Validation(error.to_string()))?;
            for notification in self.drain_notifications()? {
                let line = serde_json::to_string(&notification)?;
                writeln!(writer, "{line}")
                    .map_err(|error| AgentOsError::Validation(error.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn drain_notifications(&mut self) -> AgentOsResult<Vec<AppNotificationEnvelope>> {
        let subscriptions = self
            .subscriptions
            .iter()
            .map(|(subscription_id, cursor)| (subscription_id.clone(), cursor.clone()))
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        for (subscription_id, cursor) in subscriptions {
            let mut latest_cursor = cursor.clone();
            for mut notification in self.service.app_notifications_since(&cursor)? {
                latest_cursor = notification.cursor.clone();
                notification.subscription_id = Some(subscription_id.clone());
                output.push(notification);
            }
            self.subscriptions.insert(subscription_id, latest_cursor);
        }
        Ok(output)
    }

    fn initialize(&mut self, client: ClientConnection) -> AppResponse {
        if let Err(message) = validate_client_identity(&client) {
            return reject("invalid_client", message);
        }
        self.initialized_clients.insert(client.client_id.clone());
        AppResponse::Accepted(json!({
            "initialized": true,
            "client_id": client.client_id,
            "authority": client.authority,
        }))
    }

    fn with_initialized_client(
        &mut self,
        client: &ClientConnection,
        handle: impl FnOnce(&mut Self) -> AppResponse,
    ) -> AppResponse {
        if !self.initialized_clients.contains(&client.client_id) {
            return reject(
                "not_initialized",
                "initialize must complete before app requests".to_string(),
            );
        }
        if let Err(message) = validate_client_identity(client) {
            return reject("invalid_client", message);
        }
        handle(self)
    }

    fn subscribe(&mut self, cursor: Option<ProjectionCursor>) -> AppResponse {
        let subscription_id = format!("sub_{:016x}", self.next_subscription_id);
        self.next_subscription_id += 1;
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => match self.service.current_projection_cursor() {
                Ok(cursor) => cursor,
                Err(error) => return error_response(error),
            },
        };
        self.subscriptions
            .insert(subscription_id.clone(), cursor.clone());
        AppResponse::Accepted(json!({
            "subscription_id": subscription_id,
            "cursor": cursor,
        }))
    }

    fn unsubscribe(&mut self, subscription_id: String) -> AppResponse {
        if self.subscriptions.remove(&subscription_id).is_none() {
            return reject(
                "subscription_not_found",
                format!("subscription {subscription_id}"),
            );
        }
        AppResponse::Accepted(json!({
            "subscription_id": subscription_id,
            "unsubscribed": true,
        }))
    }
}

fn validate_client_identity(client: &ClientConnection) -> Result<(), String> {
    if client.client_id.trim().is_empty() {
        return Err("client_id is required".to_string());
    }
    if matches!(
        client.client_kind,
        ClientKind::Human | ClientKind::DesktopApp | ClientKind::TerminalUi | ClientKind::Ide
    ) && client.authority != SecurityLevel::HUMAN_ROOT
    {
        return Err("human app clients must use S0 HUMAN_ROOT authority".to_string());
    }
    Ok(())
}

fn error_response(error: AgentOsError) -> AppResponse {
    let (code, message) = match error {
        AgentOsError::Validation(message) => ("validation_failed", message),
        AgentOsError::NotFound(message) => ("not_found", message),
        AgentOsError::InvalidTransition(message) => ("invalid_transition", message),
        AgentOsError::PermissionDenied(message) => ("permission_denied", message),
        AgentOsError::ApprovalRequired(message) => ("approval_required", message),
        AgentOsError::ResourceConflict(message) => ("resource_conflict", message),
        AgentOsError::BudgetExhausted(message) => ("budget_exhausted", message),
        AgentOsError::IdempotencyConflict(message) => ("idempotency_conflict", message),
        AgentOsError::Serialization(message) => ("serialization_failed", message),
    };
    reject(code, message)
}

fn reject(code: impl Into<String>, message: impl Into<String>) -> AppResponse {
    AppResponse::Rejected {
        code: code.into(),
        message: message.into(),
    }
}

fn fallback_serialization_error(message: String) -> String {
    let value: Value = json!({
        "protocol": agent_os_sys::APP_PROTOCOL_VERSION,
        "request_id": "",
        "response": {
            "status": "Rejected",
            "body": {
                "code": "serialization_failed",
                "message": message
            }
        }
    });
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_sys::{
        AppNotification, AppNotificationEnvelope, ClientKind, StatsQuery, StatsSnapshot,
    };
    use serde_json::json;
    use std::cell::RefCell;
    use std::io::Cursor;

    #[test]
    fn rejects_requests_before_initialize() {
        let mut server = AppServer::new(FakeService::default());

        let response = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_stats".to_string(),
            client: human_client(),
            request: AppRequest::StatsRead {
                query: StatsQuery::default(),
            },
        });

        assert_eq!(response.request_id, "req_stats");
        assert_eq!(
            response.response,
            AppResponse::Rejected {
                code: "not_initialized".to_string(),
                message: "initialize must complete before app requests".to_string()
            }
        );
    }

    #[test]
    fn rejects_unsupported_protocol_version() {
        let mut server = AppServer::new(FakeService::default());

        let response = server.handle_envelope(AppRequestEnvelope {
            protocol: "agent-os.app.v0".to_string(),
            request_id: "req_init".to_string(),
            client: human_client(),
            request: AppRequest::Initialize,
        });

        assert_eq!(response.protocol, agent_os_sys::APP_PROTOCOL_VERSION);
        assert_eq!(
            response.response,
            AppResponse::Rejected {
                code: "unsupported_protocol".to_string(),
                message: "expected protocol agent-os.app.v1".to_string()
            }
        );
    }

    #[test]
    fn initializes_human_client_and_dispatches_stats_request() {
        let mut server = AppServer::new(FakeService::default());
        initialize(&mut server);

        let response = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_stats".to_string(),
            client: human_client(),
            request: AppRequest::StatsRead {
                query: StatsQuery::default(),
            },
        });

        assert_eq!(
            response.response,
            AppResponse::Accepted(json!({
                "snapshot": StatsSnapshot {
                    provider_calls: 3,
                    ..StatsSnapshot::default()
                }
            }))
        );
    }

    #[test]
    fn thread_read_response_projects_process_sessions() {
        let response = thread_read_response(ThreadReadProjection {
            thread: json!({"client_thread_id": "thread_1"}),
            turns: Vec::<Value>::new(),
            timeline: Vec::<Value>::new(),
            runtime_jobs: Vec::<Value>::new(),
            process_sessions: vec![json!({"process_id": "proc_1"})],
            artifacts: Vec::<Value>::new(),
            evidence: Vec::<Value>::new(),
            resources: Vec::<Value>::new(),
            automation_runs: Vec::<Value>::new(),
        })
        .unwrap();

        let AppResponse::Accepted(body) = response else {
            panic!("thread/read response rejected");
        };
        assert_eq!(body["process_sessions"][0]["process_id"], "proc_1");
    }

    #[test]
    fn subscribe_and_unsubscribe_are_protocol_level_state() {
        let mut server = AppServer::new(FakeService::default());
        initialize(&mut server);

        let subscribed = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_subscribe".to_string(),
            client: human_client(),
            request: AppRequest::Subscribe {
                cursor: Some(ProjectionCursor {
                    last_event_ordinal: 9,
                }),
            },
        });
        let AppResponse::Accepted(body) = subscribed.response else {
            panic!("subscribe rejected");
        };
        let subscription_id = body["subscription_id"].as_str().unwrap().to_string();
        assert_eq!(body["cursor"]["last_event_ordinal"], 9);

        let unsubscribed = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_unsubscribe".to_string(),
            client: human_client(),
            request: AppRequest::Unsubscribe { subscription_id },
        });

        assert!(matches!(unsubscribed.response, AppResponse::Accepted(_)));
    }

    #[test]
    fn subscribe_without_cursor_starts_from_current_projection_cursor() {
        let mut server = AppServer::new(FakeService::with_current_cursor(ProjectionCursor {
            last_event_ordinal: 9,
        }));
        initialize(&mut server);

        let subscribed = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_subscribe_live".to_string(),
            client: human_client(),
            request: AppRequest::Subscribe { cursor: None },
        });
        let AppResponse::Accepted(body) = subscribed.response else {
            panic!("subscribe rejected");
        };

        assert_eq!(body["cursor"]["last_event_ordinal"], 9);
        assert!(server.drain_notifications().unwrap().is_empty());
    }

    #[test]
    fn drain_notifications_replays_subscribed_cursor_and_advances() {
        let mut server = AppServer::new(FakeService::with_notifications(vec![
            notification_at(2, 1),
            notification_at(3, 2),
        ]));
        initialize(&mut server);

        let subscribed = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_subscribe".to_string(),
            client: human_client(),
            request: AppRequest::Subscribe {
                cursor: Some(ProjectionCursor {
                    last_event_ordinal: 1,
                }),
            },
        });
        let AppResponse::Accepted(body) = subscribed.response else {
            panic!("subscribe rejected");
        };
        let subscription_id = body["subscription_id"].as_str().unwrap().to_string();

        let notifications = server.drain_notifications().unwrap();

        assert_eq!(notifications.len(), 2);
        assert_eq!(
            notifications[0].subscription_id.as_deref(),
            Some(subscription_id.as_str())
        );
        assert_eq!(notifications[0].cursor.last_event_ordinal, 2);
        assert_eq!(notifications[1].cursor.last_event_ordinal, 3);
        assert!(server.drain_notifications().unwrap().is_empty());
    }

    #[test]
    fn jsonl_loop_writes_one_response_per_request_line() {
        let mut server = AppServer::new(FakeService::default());
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&AppRequestEnvelope {
                protocol: app_protocol_version(),
                request_id: "req_init".to_string(),
                client: human_client(),
                request: AppRequest::Initialize,
            })
            .unwrap(),
            serde_json::to_string(&AppRequestEnvelope {
                protocol: app_protocol_version(),
                request_id: "req_stats".to_string(),
                client: human_client(),
                request: AppRequest::StatsRead {
                    query: StatsQuery::default(),
                },
            })
            .unwrap(),
        );
        let mut output = Vec::new();

        server
            .serve_jsonl(Cursor::new(input.as_bytes()), &mut output)
            .unwrap();

        let lines = String::from_utf8(output).unwrap();
        let responses: Vec<AppResponseEnvelope> = lines
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].request_id, "req_init");
        assert_eq!(responses[1].request_id, "req_stats");
    }

    #[test]
    fn jsonl_loop_writes_notifications_after_responses() {
        let mut server =
            AppServer::new(FakeService::with_notifications(vec![notification_at(1, 7)]));
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&AppRequestEnvelope {
                protocol: app_protocol_version(),
                request_id: "req_init".to_string(),
                client: human_client(),
                request: AppRequest::Initialize,
            })
            .unwrap(),
            serde_json::to_string(&AppRequestEnvelope {
                protocol: app_protocol_version(),
                request_id: "req_subscribe".to_string(),
                client: human_client(),
                request: AppRequest::Subscribe {
                    cursor: Some(ProjectionCursor {
                        last_event_ordinal: 0,
                    }),
                },
            })
            .unwrap(),
        );
        let mut output = Vec::new();

        server
            .serve_jsonl(Cursor::new(input.as_bytes()), &mut output)
            .unwrap();

        let lines = String::from_utf8(output).unwrap();
        let lines = lines.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        let response: AppResponseEnvelope = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(response.request_id, "req_subscribe");
        let notification: AppNotificationEnvelope = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(
            notification.subscription_id.as_deref(),
            Some("sub_0000000000000001")
        );
        assert_eq!(notification.cursor.last_event_ordinal, 1);
    }

    #[test]
    fn jsonl_app_client_writes_request_envelope_and_reads_response() {
        let response = AppResponseEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_0000000000000001".to_string(),
            response: AppResponse::Accepted(json!({"initialized": true})),
        };
        let input = format!("{}\n", serde_json::to_string(&response).unwrap());
        let output = Vec::new();
        let mut client = JsonlAppClient::new(human_client(), Cursor::new(input.as_bytes()), output);

        let returned = client.request(AppRequest::Initialize).unwrap();
        let (_, output) = client.into_inner();
        let written = String::from_utf8(output).unwrap();
        let envelope: AppRequestEnvelope = serde_json::from_str(written.trim()).unwrap();

        assert_eq!(returned, response);
        assert_eq!(envelope.request_id, "req_0000000000000001");
        assert!(matches!(envelope.request, AppRequest::Initialize));
        assert_eq!(envelope.client.client_id, "human_1");
    }

    #[test]
    fn jsonl_app_client_reads_notification_after_response_line() {
        let response = AppResponseEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_0000000000000001".to_string(),
            response: AppResponse::Accepted(json!({"subscription_id": "sub_1"})),
        };
        let notification = notification_at(3, 5);
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&response).unwrap(),
            serde_json::to_string(&notification).unwrap()
        );
        let output = Vec::new();
        let mut client = JsonlAppClient::new(human_client(), Cursor::new(input.as_bytes()), output);

        let returned = client
            .request(AppRequest::Subscribe {
                cursor: Some(ProjectionCursor {
                    last_event_ordinal: 0,
                }),
            })
            .unwrap();
        let returned_notification = client.read_notification().unwrap().unwrap();

        assert_eq!(returned, response);
        assert_eq!(returned_notification.cursor.last_event_ordinal, 3);
        assert!(matches!(
            returned_notification.notification,
            AppNotification::StatsUpdated(snapshot) if snapshot.provider_calls == 5
        ));
    }

    struct FakeService {
        requests: RefCell<Vec<AppRequest>>,
        notifications: RefCell<Vec<AppNotificationEnvelope>>,
        current_cursor: ProjectionCursor,
    }

    impl Default for FakeService {
        fn default() -> Self {
            Self {
                requests: RefCell::default(),
                notifications: RefCell::default(),
                current_cursor: ProjectionCursor {
                    last_event_ordinal: 0,
                },
            }
        }
    }

    impl FakeService {
        fn with_notifications(notifications: Vec<AppNotificationEnvelope>) -> Self {
            Self {
                notifications: RefCell::new(notifications),
                ..Self::default()
            }
        }

        fn with_current_cursor(current_cursor: ProjectionCursor) -> Self {
            Self {
                current_cursor,
                ..Self::default()
            }
        }
    }

    impl AppKernelService for FakeService {
        fn handle_app_request(
            &self,
            _client: &ClientConnection,
            request: AppRequest,
        ) -> AgentOsResult<AppResponse> {
            self.requests.borrow_mut().push(request);
            Ok(AppResponse::Accepted(json!({
                "snapshot": StatsSnapshot {
                    provider_calls: 3,
                    ..StatsSnapshot::default()
                }
            })))
        }

        fn app_notifications_since(
            &self,
            cursor: &ProjectionCursor,
        ) -> AgentOsResult<Vec<AppNotificationEnvelope>> {
            Ok(self
                .notifications
                .borrow()
                .iter()
                .filter(|notification| {
                    notification.cursor.last_event_ordinal > cursor.last_event_ordinal
                })
                .cloned()
                .collect())
        }

        fn current_projection_cursor(&self) -> AgentOsResult<ProjectionCursor> {
            Ok(self.current_cursor.clone())
        }
    }

    fn initialize(server: &mut AppServer<FakeService>) {
        let response = server.handle_envelope(AppRequestEnvelope {
            protocol: app_protocol_version(),
            request_id: "req_init".to_string(),
            client: human_client(),
            request: AppRequest::Initialize,
        });
        assert!(matches!(response.response, AppResponse::Accepted(_)));
    }

    fn human_client() -> ClientConnection {
        ClientConnection {
            client_id: "human_1".to_string(),
            client_name: "Terminal".to_string(),
            client_kind: ClientKind::TerminalUi,
            authority: SecurityLevel::HUMAN_ROOT,
            connected_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }

    fn notification_at(ordinal: u64, provider_calls: u64) -> AppNotificationEnvelope {
        AppNotificationEnvelope {
            protocol: app_protocol_version(),
            subscription_id: None,
            cursor: ProjectionCursor {
                last_event_ordinal: ordinal,
            },
            notification: AppNotification::StatsUpdated(StatsSnapshot {
                provider_calls,
                ..StatsSnapshot::default()
            }),
        }
    }
}
