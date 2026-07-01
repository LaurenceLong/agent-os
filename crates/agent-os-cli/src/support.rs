use agent_os_app_server::JsonlAppClient;
#[cfg(test)]
use agent_os_kernel::Kernel;
#[cfg(test)]
use agent_os_kerneld::KernelDaemon;
#[cfg(test)]
use agent_os_store::LocalBlobStore;
#[cfg(test)]
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::{
    now_rfc3339, AgentOsError, AgentOsResult, AppRequest, AppResponse, ClientConnection,
    ClientKind, SecurityLevel,
};
use std::fs;
use std::io::{self, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub(crate) fn ensure_safe_relative_workspace_path(path: &Path, flag: &str) -> AgentOsResult<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentOsError::Validation(format!(
            "{flag} must be a relative path inside --workspace"
        )));
    }
    Ok(())
}

pub(crate) fn io_result<T>(result: io::Result<T>, context: &str) -> AgentOsResult<T> {
    result.map_err(|error| AgentOsError::Validation(format!("{context}: {error}")))
}

pub(crate) fn write_task_bundle_from_app_response(
    workspace: &Path,
    bundle_output: &Option<PathBuf>,
    bundle: &serde_json::Value,
) -> AgentOsResult<Option<String>> {
    let Some(relative_path) = bundle_output else {
        return Ok(None);
    };
    ensure_safe_relative_workspace_path(relative_path, "--bundle-output")?;
    let path = workspace.join(relative_path);
    if let Some(parent) = path.parent() {
        io_result(fs::create_dir_all(parent), "create bundle output parent")?;
    }
    let bytes = serde_json::to_vec_pretty(bundle)?;
    io_result(fs::write(&path, bytes), "write task bundle")?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[derive(Debug, Clone)]
pub(crate) struct StdioKerneldConfig {
    pub(crate) state_db: PathBuf,
    pub(crate) model_command: Option<PathBuf>,
    pub(crate) model_args: Vec<String>,
    pub(crate) provider: Option<String>,
    pub(crate) provider_config: Option<PathBuf>,
    pub(crate) max_steps: Option<u32>,
    pub(crate) max_tokens: Option<u64>,
    pub(crate) temperature: Option<String>,
}

impl StdioKerneldConfig {
    pub(crate) fn state_db(state_db: impl Into<PathBuf>) -> Self {
        Self {
            state_db: state_db.into(),
            model_command: None,
            model_args: Vec::new(),
            provider: None,
            provider_config: None,
            max_steps: None,
            max_tokens: None,
            temperature: None,
        }
    }
}

pub(crate) struct StdioKerneldAppClient {
    client: JsonlAppClient<BufReader<ChildStdout>, ChildStdin>,
    child: Child,
}

impl StdioKerneldAppClient {
    pub(crate) fn open(config: &StdioKerneldConfig) -> AgentOsResult<Self> {
        if let Some(parent) = config.state_db.parent() {
            if !parent.as_os_str().is_empty() {
                io_result(fs::create_dir_all(parent), "create state database parent")?;
            }
        }
        let kerneld = resolve_kerneld_executable()?;
        let mut command = Command::new(&kerneld);
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
        if let Some(provider) = &config.provider {
            command.arg("--provider").arg(provider);
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
                    "spawn kerneld {}: {error}",
                    kerneld.to_string_lossy()
                ))
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentOsError::Validation("kerneld stdin was not piped".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentOsError::Validation("kerneld stdout was not piped".to_string()))?;
        Ok(Self {
            client: JsonlAppClient::new(cli_client(), BufReader::new(stdout), stdin),
            child,
        })
    }

    pub(crate) fn request(&mut self, request: AppRequest) -> AgentOsResult<serde_json::Value> {
        let response = self.client.request(request)?;
        match response.response {
            AppResponse::Accepted(body) => Ok(body),
            AppResponse::Rejected { code, message } => Err(AgentOsError::Validation(format!(
                "app-server {code}: {message}"
            ))),
        }
    }
}

impl Drop for StdioKerneldAppClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) fn cli_client() -> ClientConnection {
    ClientConnection {
        client_id: "agent-os-cli".to_string(),
        client_name: "Agent-OS CLI".to_string(),
        client_kind: ClientKind::TerminalUi,
        authority: SecurityLevel::HUMAN_ROOT,
        connected_at: now_rfc3339(),
    }
}

pub(crate) fn resolve_kerneld_executable() -> AgentOsResult<PathBuf> {
    let current_exe = std::env::current_exe().map_err(|error| {
        AgentOsError::Validation(format!("resolve current executable: {error}"))
    })?;
    let current_dir = current_exe.parent().ok_or_else(|| {
        AgentOsError::Validation(format!(
            "current executable has no parent: {}",
            current_exe.to_string_lossy()
        ))
    })?;
    let direct = current_dir.join(kerneld_executable_file_name());
    if direct.exists() {
        return Ok(direct);
    }
    let cargo_test = if current_dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
        current_dir
            .parent()
            .map(|parent| parent.join(kerneld_executable_file_name()))
    } else {
        None
    };
    if let Some(candidate) = cargo_test {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(AgentOsError::Validation(format!(
        "kerneld executable not found next to {}; expected {}",
        current_exe.to_string_lossy(),
        kerneld_executable_file_name().display()
    )))
}

fn kerneld_executable_file_name() -> &'static Path {
    Path::new(if cfg!(windows) {
        "agent-os-kerneld.exe"
    } else {
        "agent-os-kerneld"
    })
}

#[cfg(test)]
pub(crate) fn open_daemon(state_db: &Option<PathBuf>) -> AgentOsResult<KernelDaemon> {
    let kernel = if let Some(path) = state_db {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                io_result(fs::create_dir_all(parent), "create state database parent")?;
            }
        }
        let store = SqliteStore::open(path)?;
        Kernel::with_replayed_store(store)?
    } else {
        Kernel::new()
    };
    attach_cli_blob_stores(kernel, state_db).and_then(KernelDaemon::try_new)
}

#[cfg(test)]
fn attach_cli_blob_stores(kernel: Kernel, state_db: &Option<PathBuf>) -> AgentOsResult<Kernel> {
    let root = match state_db {
        Some(path) => match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::env::current_dir()
                .map_err(|error| AgentOsError::Validation(format!("resolve cwd: {error}")))?,
        },
        None => std::env::current_dir()
            .map_err(|error| AgentOsError::Validation(format!("resolve cwd: {error}")))?
            .join(".agent-os"),
    };
    let blob_root = root.join("blobs");
    let artifact_blobs = LocalBlobStore::new(blob_root.join("artifacts"))?;
    let evidence_blobs = LocalBlobStore::new(blob_root.join("evidence"))?;
    Ok(kernel.with_blob_stores(artifact_blobs, evidence_blobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_kernel::{
        AttachEvidenceInput, CommitArtifactInput, RegisterGoalInput, SpawnAgentInput,
        SpawnTaskInput,
    };
    use agent_os_sys::{ArtifactType, EvidenceType};
    use serde_json::json;

    #[test]
    fn opened_daemon_persists_inline_evidence_and_artifact_blobs() {
        let root =
            std::env::temp_dir().join(format!("agent-os-cli-blob-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let state_db = root.join("state.sqlite");

        let daemon = open_daemon(&Some(state_db)).unwrap();
        let kernel = daemon.kernel().clone();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "cli-test".to_string(),
                created_by: "test".to_string(),
                title: "Blob store".to_string(),
                description: "Blob store".to_string(),
                acceptance_criteria: vec!["inline blobs can be persisted".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id.clone(),
                parent_task_id: None,
                title: "Persist blobs".to_string(),
                description: "Persist blobs".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 1,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id.clone(),
                role_profile_id: "role_worker".to_string(),
                owner: "test".to_string(),
                goal: "Persist blobs".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![root.to_string_lossy().to_string()],
            })
            .unwrap();

        let evidence = kernel
            .attach_evidence(AttachEvidenceInput {
                goal_id: goal.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
                artifact_id: None,
                evidence_type: EvidenceType::CommandLog,
                producer_agent_id: Some(agent.agent_id.clone()),
                claim: Some("inline command evidence".to_string()),
                blob_ref: None,
                content_hash: None,
                inline_bytes: Some(b"command output".to_vec()),
                metadata: json!({}),
            })
            .unwrap();
        assert!(evidence.blob_ref.is_some());

        let artifact = kernel
            .commit_artifact(CommitArtifactInput {
                goal_id: goal.goal_id,
                task_id: task.task_id,
                owner_agent_id: agent.agent_id,
                artifact_type: ArtifactType::BenchmarkResult,
                blob_ref: None,
                content_hash: None,
                inline_bytes: Some(b"benchmark result".to_vec()),
                metadata: json!({}),
                evidence_ids: vec![evidence.evidence_id],
                supersedes: None,
            })
            .unwrap();
        assert!(artifact.blob_ref.is_some());

        let _ = fs::remove_dir_all(root);
    }
}
