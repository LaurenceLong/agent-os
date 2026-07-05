use agent_os_app_server::JsonlAppClient;
use agent_os_config::AgentOsPaths;
use agent_os_sys::{
    now_rfc3339, AgentOsError, AgentOsResult, AppNotificationEnvelope, AppRequest, AppResponse,
    ClientConnection, ClientKind, SecurityLevel,
};
use serde_json::Value;
use std::fs;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Debug, Clone)]
pub struct StdioHostConfig {
    pub state_db: PathBuf,
    pub model_command: Option<PathBuf>,
    pub model_args: Vec<String>,
    pub model: Option<String>,
    pub provider_config: Option<PathBuf>,
    pub max_steps: Option<u32>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<String>,
}

impl StdioHostConfig {
    pub fn state_db(state_db: impl Into<PathBuf>) -> Self {
        Self {
            state_db: state_db.into(),
            model_command: None,
            model_args: Vec::new(),
            model: None,
            provider_config: None,
            max_steps: None,
            max_tokens: None,
            temperature: None,
        }
    }
}

pub struct StdioHostClient {
    client: JsonlAppClient<BufReader<ChildStdout>, ChildStdin>,
    child: Child,
}

impl StdioHostClient {
    pub fn open(config: &StdioHostConfig) -> AgentOsResult<Self> {
        if let Some(parent) = config.state_db.parent() {
            if !parent.as_os_str().is_empty() {
                io_result(fs::create_dir_all(parent), "create state database parent")?;
            }
        }
        let hostd = resolve_hostd_executable()?;
        let mut command = Command::new(&hostd);
        command
            .arg("--stdio")
            .arg("--state-db")
            .arg(&config.state_db);
        if let Some(model_command) = &config.model_command {
            command.arg("--model-command").arg(model_command);
            for arg in &config.model_args {
                command.arg("--model-arg").arg(arg);
            }
        }
        if let Some(model) = &config.model {
            command.arg("--model").arg(model);
        }
        if let Some(provider_config) = &config.provider_config {
            command.arg("--provider-config").arg(provider_config);
        }
        if let Some(max_steps) = config.max_steps {
            command.arg("--max-steps").arg(max_steps.to_string());
        }
        if let Some(max_tokens) = config.max_tokens {
            command.arg("--max-tokens").arg(max_tokens.to_string());
        }
        if let Some(temperature) = &config.temperature {
            command.arg("--temperature").arg(temperature);
        }
        let mut child = command
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
            client: JsonlAppClient::new(terminal_ui_client(), BufReader::new(stdout), stdin),
            child,
        })
    }

    pub fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        let response = self.client.request(request)?;
        match response.response {
            AppResponse::Accepted(body) => Ok(body),
            AppResponse::Rejected { code, message } => Err(AgentOsError::Validation(format!(
                "app-server {code}: {message}"
            ))),
        }
    }

    pub fn read_notification(&mut self) -> AgentOsResult<Option<AppNotificationEnvelope>> {
        Ok(self.client.take_pending_notification())
    }
}

impl Drop for StdioHostClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn terminal_ui_client() -> ClientConnection {
    ClientConnection {
        client_id: "agent-os-terminal-ui".to_string(),
        client_name: "Agent-OS Terminal UI".to_string(),
        client_kind: ClientKind::TerminalUi,
        authority: SecurityLevel::HUMAN_ROOT,
        connected_at: now_rfc3339(),
    }
}

pub fn default_state_db() -> AgentOsResult<PathBuf> {
    Ok(AgentOsPaths::resolve()?.default_state_db())
}

pub fn default_state_db_for_workspace(workspace: &Path) -> AgentOsResult<PathBuf> {
    Ok(AgentOsPaths::resolve()?
        .project_runtime_paths(workspace)?
        .state_db)
}

pub fn resolve_hostd_executable() -> AgentOsResult<PathBuf> {
    #[cfg(test)]
    ensure_cargo_test_hostd_executable()?;

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

fn io_result<T>(result: io::Result<T>, context: &str) -> AgentOsResult<T> {
    result.map_err(|error| AgentOsError::Validation(format!("{context}: {error}")))
}

#[cfg(test)]
fn ensure_cargo_test_hostd_executable() -> AgentOsResult<()> {
    use std::sync::OnceLock;

    static HOSTD_BUILD: OnceLock<Result<(), String>> = OnceLock::new();
    match HOSTD_BUILD
        .get_or_init(|| build_cargo_test_hostd_executable().map_err(|error| error.to_string()))
    {
        Ok(()) => Ok(()),
        Err(message) => Err(AgentOsError::Validation(message.clone())),
    }
}

#[cfg(test)]
fn build_cargo_test_hostd_executable() -> AgentOsResult<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            AgentOsError::Validation(format!(
                "resolve workspace root from {}",
                manifest_dir.to_string_lossy()
            ))
        })?;
    let output = Command::new(cargo)
        .arg("build")
        .arg("-p")
        .arg("agent-os-host")
        .arg("--bin")
        .arg("agent-os-hostd")
        .current_dir(workspace_root)
        .output()
        .map_err(|error| AgentOsError::Validation(format!("build hostd for tests: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(AgentOsError::Validation(format!(
        "build hostd for tests failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn hostd_executable_file_name() -> &'static Path {
    Path::new(if cfg!(windows) {
        "agent-os-hostd.exe"
    } else {
        "agent-os-hostd"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_host_config_defaults_to_no_runtime_model() {
        let config = StdioHostConfig::state_db("state.sqlite");

        assert_eq!(config.state_db, PathBuf::from("state.sqlite"));
        assert_eq!(config.max_steps, None);
        assert!(config.model_args.is_empty());
    }

    #[test]
    fn terminal_client_uses_human_root_terminal_identity() {
        let client = terminal_ui_client();

        assert_eq!(client.client_kind, ClientKind::TerminalUi);
        assert_eq!(client.authority, SecurityLevel::HUMAN_ROOT);
    }
}
