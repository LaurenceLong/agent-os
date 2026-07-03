use crate::args::StatusOptions;
use crate::support::default_state_db;
use agent_os_app_server::JsonlAppClient;
use agent_os_sys::*;
use serde_json::{json, Value};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub(crate) fn run_status(options: &StatusOptions) -> AgentOsResult<Value> {
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(default_state_db)?;
    let mut app_client = StdioStatusClient::open(&state_db)?;
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

struct StdioStatusClient {
    client: JsonlAppClient<BufReader<ChildStdout>, ChildStdin>,
    child: Child,
}

impl StdioStatusClient {
    fn open(state_db: &Path) -> AgentOsResult<Self> {
        let hostd = resolve_hostd_executable()?;
        let mut child = Command::new(&hostd)
            .arg("--stdio")
            .arg("--state-db")
            .arg(state_db)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                AgentOsError::Validation(format!(
                    "spawn hostd {}: {error}",
                    hostd.to_string_lossy()
                ))
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentOsError::Validation("hostd stdin was not piped".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentOsError::Validation("hostd stdout was not piped".to_string()))?;
        Ok(Self {
            client: JsonlAppClient::new(cli_client(), BufReader::new(stdout), stdin),
            child,
        })
    }
}

impl StatusAppClient for StdioStatusClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        let response = self.client.request(request)?;
        match response.response {
            AppResponse::Accepted(body) => Ok(body),
            AppResponse::Rejected { code, message } => Err(AgentOsError::Validation(format!(
                "app-server {code}: {message}"
            ))),
        }
    }
}

impl Drop for StdioStatusClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn resolve_hostd_executable() -> AgentOsResult<PathBuf> {
    let current_exe = std::env::current_exe().map_err(|error| {
        AgentOsError::Validation(format!("resolve current executable: {error}"))
    })?;
    let current_dir = current_exe.parent().ok_or_else(|| {
        AgentOsError::Validation(format!(
            "current executable has no parent: {}",
            current_exe.to_string_lossy()
        ))
    })?;
    let direct = current_dir.join(hostd_executable_file_name());
    if direct.exists() {
        return Ok(direct);
    }
    let cargo_test = if current_dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
        current_dir
            .parent()
            .map(|parent| parent.join(hostd_executable_file_name()))
    } else {
        None
    };
    if let Some(candidate) = cargo_test {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(AgentOsError::Validation(format!(
        "hostd executable not found next to {}; expected {}",
        current_exe.to_string_lossy(),
        hostd_executable_file_name().display()
    )))
}

fn hostd_executable_file_name() -> &'static Path {
    Path::new(if cfg!(windows) {
        "agent-os-hostd.exe"
    } else {
        "agent-os-hostd"
    })
}

fn cli_client() -> ClientConnection {
    ClientConnection {
        client_id: "agent-os-cli".to_string(),
        client_name: "Agent-OS CLI".to_string(),
        client_kind: ClientKind::TerminalUi,
        authority: SecurityLevel::HUMAN_ROOT,
        connected_at: now_rfc3339(),
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
                    }))
                }
                other => panic!("unexpected status request: {other:?}"),
            }
        }
    }
}
