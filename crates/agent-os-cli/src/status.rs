use crate::args::StatusOptions;
use crate::support::{default_state_db, StdioHostAppClient, StdioHostConfig};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::path::Path;

pub(crate) fn run_status(options: &StatusOptions) -> AgentOsResult<Value> {
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(default_state_db)?;
    let mut app_client = StdioHostAppClient::open(&StdioHostConfig::state_db(state_db.clone()))?;
    status_from_app_client(&mut app_client, options, &state_db)
}

trait StatusAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value>;
}

fn status_from_app_client(
    app_client: &mut impl StatusAppClient,
    options: &StatusOptions,
    state_db: &Path,
) -> AgentOsResult<Value> {
    let _ = app_client.request(AppRequest::Initialize)?;
    let stats = app_request(
        app_client,
        AppRequest::StatsRead {
            query: StatsQuery::default(),
        },
    )?["snapshot"]
        .clone();
    if let Some(thread_id) = &options.thread_id {
        let body = app_request(
            app_client,
            AppRequest::ThreadRead {
                client_thread_id: thread_id.clone(),
            },
        )?;
        return Ok(json!({
            "state_db": state_db.to_string_lossy(),
            "thread": body["thread"],
            "turns": body["turns"],
            "timeline": body["timeline"],
            "runtime_jobs": body["runtime_jobs"],
            "process_sessions": body["process_sessions"],
            "stats": stats,
        }));
    }
    let body = app_request(app_client, AppRequest::ThreadList { archived: None })?;
    Ok(json!({
        "state_db": state_db.to_string_lossy(),
        "threads": body["threads"],
        "stats": stats,
    }))
}

fn app_request(client: &mut impl StatusAppClient, request: AppRequest) -> AgentOsResult<Value> {
    client.request(request)
}

impl StatusAppClient for StdioHostAppClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        StdioHostAppClient::request(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lists_client_thread_projection_from_app_server_contract() {
        let state_db = std::path::PathBuf::from("state.sqlite");
        let mut client = FakeStatusClient::default();

        let output = status_from_app_client(
            &mut client,
            &StatusOptions {
                state_db: Some(state_db.clone()),
                thread_id: None,
            },
            &state_db,
        )
        .unwrap();

        assert_eq!(output["threads"][0]["client_thread_id"], "thread_1");
        assert!(output["stats"].is_object());
    }

    #[test]
    fn status_reads_single_thread_projection_from_app_server_contract() {
        let state_db = std::path::PathBuf::from("state.sqlite");
        let mut client = FakeStatusClient::default();

        let output = status_from_app_client(
            &mut client,
            &StatusOptions {
                state_db: Some(state_db.clone()),
                thread_id: Some("thread_1".to_string()),
            },
            &state_db,
        )
        .unwrap();

        assert_eq!(output["thread"]["client_thread_id"], "thread_1");
        assert!(!output["timeline"].as_array().unwrap().is_empty());
        assert_eq!(output["process_sessions"][0]["process_id"], "proc_1");
    }

    #[test]
    fn status_formats_projection_from_app_client_contract() {
        let state_db = std::path::PathBuf::from("state.sqlite");
        let mut client = FakeStatusClient::default();

        let output = status_from_app_client(
            &mut client,
            &StatusOptions {
                state_db: Some(state_db.clone()),
                thread_id: None,
            },
            &state_db,
        )
        .unwrap();

        assert_eq!(output["state_db"], state_db.to_string_lossy().to_string());
        assert_eq!(output["threads"][0]["client_thread_id"], "thread_1");
        assert_eq!(output["stats"]["provider_calls"], 2);
        assert_eq!(
            client.requests,
            vec!["initialize", "stats/read", "thread/list"]
        );
    }

    #[derive(Default)]
    struct FakeStatusClient {
        requests: Vec<&'static str>,
    }

    impl StatusAppClient for FakeStatusClient {
        fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
            match request {
                AppRequest::Initialize => {
                    self.requests.push("initialize");
                    Ok(json!({"initialized": true}))
                }
                AppRequest::StatsRead { .. } => {
                    self.requests.push("stats/read");
                    Ok(json!({"snapshot": {"provider_calls": 2}}))
                }
                AppRequest::ThreadList { .. } => {
                    self.requests.push("thread/list");
                    Ok(json!({"threads": [{"client_thread_id": "thread_1"}]}))
                }
                AppRequest::ThreadRead { .. } => {
                    self.requests.push("thread/read");
                    Ok(json!({
                        "thread": {"client_thread_id": "thread_1"},
                        "turns": [],
                        "timeline": [{"item_id": "item_1"}],
                        "runtime_jobs": [],
                        "process_sessions": [{"process_id": "proc_1"}],
                    }))
                }
                other => panic!("unexpected status request: {other:?}"),
            }
        }
    }
}
