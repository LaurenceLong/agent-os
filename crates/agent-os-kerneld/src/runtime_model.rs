use crate::KernelDaemon;
use agent_os_sys::{AgentOsError, AgentOsResult, AttachMode};
use agent_os_thread::{
    ExternalProcessModelClient, LlmApiStyle, ModelClient, ModelTurnRequest, ModelTurnResponse,
    OpenAiModelClient, RuntimeConfig, RuntimeJob,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRuntimeModelConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeModelConfig {
    pub provider: Option<String>,
    pub config_path: Option<PathBuf>,
    pub max_steps: u32,
    pub max_tokens: Option<u64>,
    pub temperature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonRuntimeModelConfig {
    External(ExternalRuntimeModelConfig),
    Provider(ProviderRuntimeModelConfig),
}

impl KernelDaemon {
    pub fn with_runtime_model_config(mut self, config: DaemonRuntimeModelConfig) -> Self {
        self.runtime_model_config = Some(config);
        self
    }

    pub(crate) fn has_runtime_model_config(&self) -> bool {
        self.runtime_model_config.is_some()
    }

    pub fn spawn_next_configured_runtime_job_worker(&self) -> AgentOsResult<Option<String>> {
        let Some(runtime_job_id) = self.next_queued_runtime_job_id()? else {
            return Ok(None);
        };
        self.spawn_configured_runtime_job_worker(&runtime_job_id)?;
        Ok(Some(runtime_job_id))
    }

    pub(crate) fn spawn_configured_runtime_job_worker(
        &self,
        runtime_job_id: &str,
    ) -> AgentOsResult<()> {
        let job = self.runtime_job(runtime_job_id)?;
        let Some(config) = self.runtime_model_config.clone() else {
            return Err(agent_os_sys::AgentOsError::Validation(
                "kerneld runtime model config is not configured".to_string(),
            ));
        };
        let worker = config.worker(self, &job)?;
        self.spawn_runtime_job_worker(runtime_job_id, worker.model_client, worker.runtime_config)
    }
}

impl DaemonRuntimeModelConfig {
    fn worker(
        &self,
        daemon: &KernelDaemon,
        job: &RuntimeJob,
    ) -> AgentOsResult<ConfiguredRuntimeWorker> {
        match self {
            Self::External(config) => Ok(ConfiguredRuntimeWorker {
                runtime_config: runtime_config(job, config.max_steps, Some(job.model.clone())),
                model_client: DaemonRuntimeModelClient::External(ExternalProcessModelClient::new(
                    config.program.clone(),
                    config.args.clone(),
                )),
            }),
            Self::Provider(config) => {
                let provider_config = GlobalProviderConfig::load(config.config_path.as_ref())?;
                let provider = provider_config.resolve(config.provider.as_deref())?;
                daemon.register_model_alias(
                    &provider.model,
                    &provider.name,
                    &provider.model,
                    json!({
                        "streaming": true,
                        "tool_calling": true,
                        "reasoning": true,
                        "image_input": false,
                        "structured_output": true
                    }),
                    &job.provider_profile,
                )?;
                let mut client = OpenAiModelClient::new(provider.api_key, provider.model.clone())
                    .with_api_base(provider.base_url)
                    .with_api_style(provider.api_style);
                if let Some(max_tokens) = config.max_tokens {
                    client = client.with_max_tokens(max_tokens);
                }
                if let Some(temperature) = &config.temperature {
                    client = client.with_temperature(temperature.parse().map_err(|_| {
                        AgentOsError::Validation(
                            "provider runtime temperature must be a number".to_string(),
                        )
                    })?);
                }
                Ok(ConfiguredRuntimeWorker {
                    runtime_config: runtime_config(
                        job,
                        config.max_steps,
                        Some(provider.model.clone()),
                    ),
                    model_client: DaemonRuntimeModelClient::OpenAi(client),
                })
            }
        }
    }
}

fn runtime_config(
    job: &RuntimeJob,
    max_steps: u32,
    requested_model_alias: Option<String>,
) -> RuntimeConfig {
    RuntimeConfig {
        workspace_root: PathBuf::from(&job.workspace),
        attach_mode: AttachMode::WorkspaceWrite,
        max_steps,
        requested_model_alias,
        tool_risk_ceiling: 4,
        auto_commit_patch_artifacts: true,
        fail_on_process_nonzero: false,
    }
}

struct ConfiguredRuntimeWorker {
    runtime_config: RuntimeConfig,
    model_client: DaemonRuntimeModelClient,
}

enum DaemonRuntimeModelClient {
    External(ExternalProcessModelClient),
    OpenAi(OpenAiModelClient),
}

impl ModelClient for DaemonRuntimeModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        match self {
            Self::External(client) => client.next(request),
            Self::OpenAi(client) => client.next(request),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GlobalProviderConfig {
    default_provider: String,
    providers: BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    api_key: String,
    base_url: String,
    model: String,
    api_style: String,
}

struct ResolvedProvider {
    name: String,
    api_key: String,
    base_url: String,
    model: String,
    api_style: LlmApiStyle,
}

impl GlobalProviderConfig {
    fn load(explicit_path: Option<&PathBuf>) -> AgentOsResult<Self> {
        let path = match explicit_path {
            Some(path) => path.clone(),
            None => global_provider_config_path()?,
        };
        let content = fs::read_to_string(&path).map_err(|error| {
            AgentOsError::Validation(format!(
                "read global provider config {}: {error}",
                path.display()
            ))
        })?;
        let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
        let config: Self = serde_json::from_str(content).map_err(|error| {
            AgentOsError::Validation(format!(
                "parse global provider config {}: {error}",
                path.display()
            ))
        })?;
        config.validate(&path)?;
        Ok(config)
    }

    fn resolve(&self, provider_name: Option<&str>) -> AgentOsResult<ResolvedProvider> {
        let name = provider_name.unwrap_or(&self.default_provider);
        let provider = self.providers.get(name).ok_or_else(|| {
            AgentOsError::Validation(format!("global provider config has no provider `{name}`"))
        })?;
        Ok(ResolvedProvider {
            name: name.to_string(),
            api_key: provider.api_key.clone(),
            base_url: provider.base_url.clone(),
            model: provider.model.clone(),
            api_style: LlmApiStyle::from_value(&provider.api_style)?,
        })
    }

    fn validate(&self, path: &std::path::Path) -> AgentOsResult<()> {
        if self.default_provider.trim().is_empty() {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} must set default_provider",
                path.display()
            )));
        }
        if self.providers.is_empty() {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} must define at least one provider",
                path.display()
            )));
        }
        for (name, provider) in &self.providers {
            if name.trim().is_empty()
                || provider.api_key.trim().is_empty()
                || provider.base_url.trim().is_empty()
                || provider.model.trim().is_empty()
                || provider.api_style.trim().is_empty()
            {
                return Err(AgentOsError::Validation(format!(
                    "global provider config {} has an incomplete provider entry",
                    path.display()
                )));
            }
            LlmApiStyle::from_value(&provider.api_style)?;
        }
        if !self.providers.contains_key(&self.default_provider) {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} default_provider `{}` is not defined",
                path.display(),
                self.default_provider
            )));
        }
        Ok(())
    }
}

fn global_provider_config_path() -> AgentOsResult<PathBuf> {
    let config_dir = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                AgentOsError::Validation(
                    "APPDATA is required to locate global provider config".to_string(),
                )
            })?
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .ok_or_else(|| {
                AgentOsError::Validation(
                    "XDG_CONFIG_HOME is required to locate global provider config".to_string(),
                )
            })?
    };
    Ok(config_dir.join("agent-os").join("providers.json"))
}

#[cfg(test)]
mod tests {
    use crate::{
        AppServer, DaemonRuntimeModelConfig, ExternalRuntimeModelConfig, KernelDaemon,
        ProviderRuntimeModelConfig,
    };
    use agent_os_sys::*;
    use serde_json::json;
    use serde_json::Value;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::thread::JoinHandle;

    #[test]
    fn configured_daemon_autostarts_runtime_worker_after_turn_start() {
        let workspace = temp_workspace("configured-runtime-worker");
        fs::create_dir_all(&workspace).unwrap();
        let model_program = compile_external_model(&workspace);
        let daemon = KernelDaemon::in_memory().with_runtime_model_config(
            DaemonRuntimeModelConfig::External(ExternalRuntimeModelConfig {
                program: model_program,
                args: Vec::new(),
                max_steps: 16,
            }),
        );
        let mut server = initialized_server(daemon.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "run configured runtime worker".to_string(),
            },
        );
        let shutdown = daemon.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(
            shutdown.failed_runtime_workers.is_empty(),
            "{:?}",
            shutdown.failed_runtime_workers
        );
        assert_eq!(
            fs::read_to_string(workspace.join("configured.md")).unwrap(),
            "configured daemon worker\n"
        );
        let read = request(
            &mut server,
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert_eq!(read["runtime_jobs"][0]["status"], "completed", "{read:#}");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn configured_daemon_builds_provider_client_from_provider_config() {
        let workspace = temp_workspace("configured-provider-runtime-worker");
        fs::create_dir_all(&workspace).unwrap();
        let mock_provider = MockOpenAiServer::start(vec![
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_patch",
                            "type": "function",
                            "function": {
                                "name": "apply_patch",
                                "arguments": "{\"patch\":\"*** Begin Patch\\n*** Add File: provider-configured.md\\n+provider configured daemon worker\\n*** End Patch\\n\"}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            }),
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "complete",
                        "tool_calls": [{
                            "id": "call_final",
                            "type": "function",
                            "function": {
                                "name": "submit_final",
                                "arguments": "{\"summary\":\"provider configured daemon worker complete\",\"evidence_map\":[{\"claim\":\"provider configured file was written\",\"evidence_refs\":[\"__FIRST_EVIDENCE_ID__\"]}]}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            }),
        ]);
        let provider_config_path = workspace.join("providers.json");
        fs::write(
            &provider_config_path,
            json!({
                "default_provider": "mock",
                "providers": {
                    "mock": {
                        "api_key": "test-key",
                        "base_url": mock_provider.base_url,
                        "model": "mock-provider-model",
                        "api_style": "openai-compatible"
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let daemon = KernelDaemon::in_memory().with_runtime_model_config(
            DaemonRuntimeModelConfig::Provider(ProviderRuntimeModelConfig {
                provider: Some("mock".to_string()),
                config_path: Some(provider_config_path),
                max_steps: 16,
                max_tokens: Some(128),
                temperature: Some("0.0".to_string()),
            }),
        );
        let mut server = initialized_server(daemon.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "run provider configured runtime worker".to_string(),
            },
        );
        let shutdown = daemon.shutdown().unwrap();
        mock_provider.join();

        assert!(
            shutdown.failed_runtime_workers.is_empty(),
            "{:?}",
            shutdown.failed_runtime_workers
        );
        assert_eq!(
            fs::read_to_string(workspace.join("provider-configured.md")).unwrap(),
            "provider configured daemon worker\n"
        );
        let read = request(
            &mut server,
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert_eq!(read["runtime_jobs"][0]["status"], "completed", "{read:#}");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn configured_provider_runtime_preserves_nonzero_command_result() {
        let workspace = temp_workspace("configured-provider-nonzero-worker");
        fs::create_dir_all(&workspace).unwrap();
        let current_exe = std::env::current_exe().unwrap();
        let mock_provider = MockOpenAiServer::start(vec![
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_nonzero",
                            "type": "function",
                            "function": {
                                "name": "run_command",
                                "arguments": serde_json::to_string(&json!({
                                    "program": current_exe,
                                    "args": ["--agent-os-nonzero-probe"]
                                })).unwrap()
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            }),
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "captured nonzero command result",
                        "tool_calls": [{
                            "id": "call_final",
                            "type": "function",
                            "function": {
                                "name": "submit_final",
                                "arguments": "{\"summary\":\"nonzero command result preserved\",\"evidence_map\":[{\"claim\":\"nonzero command result was captured\",\"evidence_refs\":[\"__FIRST_EVIDENCE_ID__\"]}]}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1}
            }),
        ]);
        let provider_config_path = workspace.join("providers.json");
        fs::write(
            &provider_config_path,
            json!({
                "default_provider": "mock",
                "providers": {
                    "mock": {
                        "api_key": "test-key",
                        "base_url": mock_provider.base_url,
                        "model": "mock-provider-model",
                        "api_style": "openai-compatible"
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let daemon = KernelDaemon::in_memory().with_runtime_model_config(
            DaemonRuntimeModelConfig::Provider(ProviderRuntimeModelConfig {
                provider: Some("mock".to_string()),
                config_path: Some(provider_config_path),
                max_steps: 16,
                max_tokens: Some(128),
                temperature: Some("0.0".to_string()),
            }),
        );
        let mut server = initialized_server(daemon.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "capture a nonzero command result".to_string(),
            },
        );
        let shutdown = daemon.shutdown().unwrap();

        assert!(
            shutdown.failed_runtime_workers.is_empty(),
            "{:?}",
            shutdown.failed_runtime_workers
        );
        mock_provider.join();
        let read = request(
            &mut server,
            AppRequest::ThreadRead {
                client_thread_id: thread_id,
            },
        );
        assert_eq!(read["runtime_jobs"][0]["status"], "completed");
        fs::remove_dir_all(workspace).unwrap();
    }

    fn initialized_server(daemon: KernelDaemon) -> AppServer<KernelDaemon> {
        let mut server = AppServer::new(daemon);
        request(&mut server, AppRequest::Initialize);
        server
    }

    fn start_thread(server: &mut AppServer<KernelDaemon>, workspace: &Path) -> String {
        let body = request(
            server,
            AppRequest::ThreadStart {
                goal: "Configured runtime worker".to_string(),
                workspace: Some(workspace.to_string_lossy().to_string()),
            },
        );
        body["thread"]["client_thread_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn request(server: &mut AppServer<KernelDaemon>, request: AppRequest) -> Value {
        let response = server.handle_envelope(AppRequestEnvelope {
            request_id: new_id("req_"),
            client: ClientConnection {
                client_id: "test-client".to_string(),
                client_name: "Test Client".to_string(),
                client_kind: ClientKind::TerminalUi,
                authority: SecurityLevel::HUMAN_ROOT,
                connected_at: now_rfc3339(),
            },
            request,
        });
        match response.response {
            AppResponse::Accepted(body) => body,
            AppResponse::Rejected { code, message } => {
                panic!("app request rejected: {code}: {message}")
            }
        }
    }

    fn compile_external_model(workspace: &Path) -> PathBuf {
        let source_path = workspace.join("configured_model.rs");
        let model_program =
            workspace.join(format!("configured_model{}", std::env::consts::EXE_SUFFIX));
        let mut source = fs::File::create(&source_path).unwrap();
        source
            .write_all(
                br##"
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: configured.md\\n+configured daemon worker\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"configured daemon worker wrote file through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"configured daemon worker complete\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"configured daemon worker wrote file\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                evidence_id
            );
        }
    }
}

fn step_index(input: &str) -> u32 {
    let marker = "\"step_index\":";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().unwrap()
}

fn json_string(input: &str, field: &str) -> String {
    let marker = format!("\"{}\":\"", field);
    let start = input.find(&marker).unwrap() + marker.len();
    let bytes = input.as_bytes();
    let mut index = start;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            break;
        }
        index += 1;
    }
    input[start..index].to_string()
}

fn first_evidence_id(input: &str) -> String {
    let marker = "\"evidence_ids\":[\"";
    let start = input.find(marker).unwrap() + marker.len();
    let rest = &input[start..];
    let end = rest.find('"').unwrap();
    rest[..end].to_string()
}
"##,
            )
            .unwrap();
        drop(source);
        let output = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&model_program)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "rustc failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        model_program
    }

    fn temp_workspace(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            new_id("case_")
        ))
    }

    struct MockOpenAiServer {
        base_url: String,
        handle: JoinHandle<()>,
    }

    impl MockOpenAiServer {
        fn start(responses: Vec<Value>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base_url = format!("http://{}", listener.local_addr().unwrap());
            let handle = std::thread::spawn(move || {
                for response in responses {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_http_request(&mut stream);
                    let mut body = response.to_string();
                    if body.contains("__FIRST_EVIDENCE_ID__") {
                        body = body.replace("__FIRST_EVIDENCE_ID__", &first_evidence_id(&request));
                    }
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .unwrap();
                }
            });
            Self { base_url, handle }
        }

        fn join(self) {
            self.handle.join().unwrap();
        }
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let header_end = loop {
            let mut chunk = [0u8; 4096];
            let read = stream.read(&mut chunk).unwrap();
            assert_ne!(read, 0, "mock provider connection closed before headers");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(position) = buffer.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while buffer.len() < header_end + content_length {
            let mut chunk = [0u8; 4096];
            let read = stream.read(&mut chunk).unwrap();
            assert_ne!(read, 0, "mock provider connection closed before body");
            buffer.extend_from_slice(&chunk[..read]);
        }
        String::from_utf8_lossy(&buffer).to_string()
    }

    fn first_evidence_id(input: &str) -> String {
        let start = input.find("evd_").unwrap();
        let end = input[start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|offset| start + offset)
            .unwrap_or(input.len());
        input[start..end].to_string()
    }
}
