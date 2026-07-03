use crate::args::{ProcessAction, ProcessOptions};
use crate::support::{default_state_db, StdioHostAppClient, StdioHostConfig};
use agent_os_sys::{AgentOsResult, AppRequest};
use serde_json::{json, Value};
use std::path::Path;

pub(crate) fn run_process(options: &ProcessOptions) -> AgentOsResult<Value> {
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(default_state_db)?;
    let mut app_client = StdioHostAppClient::open(&StdioHostConfig::state_db(state_db.clone()))?;
    process_from_app_client(&mut app_client, options, &state_db)
}

trait ProcessAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

impl ProcessAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

fn process_from_app_client(
    app_client: &mut impl ProcessAppClient,
    options: &ProcessOptions,
    state_db: &Path,
) -> AgentOsResult<Value> {
    let _ = app_client.request(AppRequest::Initialize)?;
    let body = app_client.request(match options.action {
        ProcessAction::Stop => AppRequest::ProcessStop {
            process_id: options.process_id.clone(),
            reason: options.reason.clone(),
        },
        ProcessAction::Kill => AppRequest::ProcessKill {
            process_id: options.process_id.clone(),
            reason: options.reason.clone(),
        },
    })?;
    Ok(json!({
        "state_db": state_db.to_string_lossy(),
        "action": process_action_name(options.action),
        "process_session": body["process_session"],
    }))
}

fn process_action_name(action: ProcessAction) -> &'static str {
    match action {
        ProcessAction::Stop => "stop",
        ProcessAction::Kill => "kill",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{ProcessAction, ProcessOptions};
    use agent_os_sys::AgentOsError;
    use std::path::PathBuf;

    #[test]
    fn process_stop_uses_app_server_cleanup_contract() {
        let state_db = PathBuf::from("state.sqlite");
        let mut client = FakeProcessClient::default();

        let output = process_from_app_client(
            &mut client,
            &ProcessOptions {
                action: ProcessAction::Stop,
                state_db: Some(state_db.clone()),
                process_id: "proc_1".to_string(),
                reason: Some("test stop".to_string()),
            },
            &state_db,
        )
        .unwrap();

        assert_eq!(output["action"], "stop");
        assert_eq!(output["process_session"]["process_id"], "proc_1");
        assert_eq!(output["process_session"]["state"], "interrupted");
        assert_eq!(client.requests, vec!["initialize", "process/stop"]);
    }

    #[test]
    fn process_kill_uses_app_server_cleanup_contract() {
        let state_db = PathBuf::from("state.sqlite");
        let mut client = FakeProcessClient::default();

        let output = process_from_app_client(
            &mut client,
            &ProcessOptions {
                action: ProcessAction::Kill,
                state_db: Some(state_db.clone()),
                process_id: "proc_2".to_string(),
                reason: None,
            },
            &state_db,
        )
        .unwrap();

        assert_eq!(output["action"], "kill");
        assert_eq!(output["process_session"]["process_id"], "proc_2");
        assert_eq!(output["process_session"]["state"], "terminated");
        assert_eq!(client.requests, vec!["initialize", "process/kill"]);
    }

    #[derive(Default)]
    struct FakeProcessClient {
        requests: Vec<&'static str>,
    }

    impl ProcessAppClient for FakeProcessClient {
        fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
            match request {
                AppRequest::Initialize => {
                    self.requests.push("initialize");
                    Ok(json!({"initialized": true}))
                }
                AppRequest::ProcessStop { process_id, reason } => {
                    self.requests.push("process/stop");
                    assert_eq!(reason.as_deref(), Some("test stop"));
                    Ok(json!({
                        "process_session": {
                            "process_id": process_id,
                            "state": "interrupted"
                        }
                    }))
                }
                AppRequest::ProcessKill { process_id, reason } => {
                    self.requests.push("process/kill");
                    assert!(reason.is_none());
                    Ok(json!({
                        "process_session": {
                            "process_id": process_id,
                            "state": "terminated"
                        }
                    }))
                }
                other => Err(AgentOsError::Validation(format!(
                    "unexpected request: {other:?}"
                ))),
            }
        }
    }
}
