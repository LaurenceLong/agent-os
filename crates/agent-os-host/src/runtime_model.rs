use crate::AgentOsHost;
use agent_os_config::{ProviderCatalog, ResolvedAgentOsConfig};
use agent_os_sys::{AgentOsError, AgentOsResult, AttachMode};
use agent_os_thread::{
    ExternalProcessModelClient, ModelClient, ModelTurnRequest, ModelTurnResponse,
    OpenAiModelClient, RuntimeConfig, RuntimeJob,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRuntimeModelConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeModelConfig {
    pub model: Option<String>,
    pub config_path: Option<PathBuf>,
    pub max_steps: u32,
    pub max_tokens: Option<u64>,
    pub temperature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRuntimeModelConfig {
    External(ExternalRuntimeModelConfig),
    Provider(ProviderRuntimeModelConfig),
}

impl AgentOsHost {
    pub fn with_runtime_model_config(mut self, config: HostRuntimeModelConfig) -> Self {
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
                "hostd runtime model config is not configured".to_string(),
            ));
        };
        let worker = config.worker(self, &job)?;
        self.spawn_runtime_job_worker(runtime_job_id, worker.model_client, worker.runtime_config)
    }
}

impl HostRuntimeModelConfig {
    fn worker(
        &self,
        host: &AgentOsHost,
        job: &RuntimeJob,
    ) -> AgentOsResult<ConfiguredRuntimeWorker> {
        match self {
            Self::External(config) => Ok(ConfiguredRuntimeWorker {
                runtime_config: runtime_config(job, config.max_steps, Some(job.model.clone())),
                model_client: HostRuntimeModelClient::External(ExternalProcessModelClient::new(
                    config.program.clone(),
                    config.args.clone(),
                )),
            }),
            Self::Provider(config) => {
                let provider_config = match config.config_path.as_ref() {
                    Some(path) => ProviderCatalog::load_from_path(path)?,
                    None => ResolvedAgentOsConfig::load(Some(Path::new(&job.workspace)))?.providers,
                };
                let model = provider_config.resolve(config.model.as_deref())?;
                host.register_model_alias(
                    &model.id,
                    &model.provider_id,
                    &model.name,
                    model.capabilities.clone(),
                    model.limit.clone(),
                    &job.provider_profile,
                )?;
                let mut client = OpenAiModelClient::new(model.api_key, model.name.clone())
                    .with_api_base(model.base_url)
                    .with_endpoint(model.endpoint)
                    .with_model_options(model.options.clone());
                if let Some(timeout_ms) = model.timeout_ms {
                    client = client.with_request_timeout(Duration::from_millis(timeout_ms));
                }
                if let Some(max_tokens) = config.max_tokens {
                    client = client.with_max_tokens(max_tokens);
                } else {
                    client = client.with_max_tokens(model.limit.output);
                }
                if let Some(temperature) = &config.temperature {
                    client = client.with_temperature(temperature.parse().map_err(|_| {
                        AgentOsError::Validation(
                            "provider runtime temperature must be a number".to_string(),
                        )
                    })?);
                }
                Ok(ConfiguredRuntimeWorker {
                    runtime_config: runtime_config(job, config.max_steps, Some(model.id.clone())),
                    model_client: HostRuntimeModelClient::OpenAi(client),
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
    model_client: HostRuntimeModelClient,
}

enum HostRuntimeModelClient {
    External(ExternalProcessModelClient),
    OpenAi(OpenAiModelClient),
}

impl ModelClient for HostRuntimeModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        match self {
            Self::External(client) => client.next(request),
            Self::OpenAi(client) => client.next(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AgentOsHost, AppServer, ExternalRuntimeModelConfig, HostRuntimeModelConfig,
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
    fn configured_host_autostarts_runtime_worker_after_turn_start() {
        let workspace = temp_workspace("configured-runtime-worker");
        fs::create_dir_all(&workspace).unwrap();
        let model_program = compile_external_model(&workspace);
        let host = AgentOsHost::in_memory().with_runtime_model_config(
            HostRuntimeModelConfig::External(ExternalRuntimeModelConfig {
                program: model_program,
                args: Vec::new(),
                max_steps: 16,
            }),
        );
        let mut server = initialized_server(host.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "run configured runtime worker".to_string(),
            },
        );
        let shutdown = host.shutdown().unwrap();

        assert_eq!(shutdown.joined_runtime_workers, 1);
        assert!(
            shutdown.failed_runtime_workers.is_empty(),
            "{:?}",
            shutdown.failed_runtime_workers
        );
        assert_eq!(
            fs::read_to_string(workspace.join("configured.md")).unwrap(),
            "configured host worker\n"
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
    fn configured_host_builds_provider_client_from_provider_config() {
        let workspace = temp_workspace("configured-provider-runtime-worker");
        fs::create_dir_all(&workspace).unwrap();
        let mock_provider = MockOpenAiServer::start_expect_model_and_request_snippets(
            "wire-mock-model",
            vec![
                "\"reasoningEffort\":\"high\"",
                "\"reasoningSummary\":\"auto\"",
                "\"max_tokens\":2048",
            ],
            vec![
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
                                    "arguments": "{\"patch\":\"*** Begin Patch\\n*** Add File: provider-configured.md\\n+provider configured host worker\\n*** End Patch\\n\"}"
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
                                    "arguments": "{\"summary\":\"provider configured host worker complete\",\"evidence_map\":[{\"claim\":\"provider configured file was written\",\"evidence_refs\":[\"__FIRST_EVIDENCE_ID__\"]}]}"
                                }
                            }]
                        }
                    }],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1}
                }),
            ],
        );
        let provider_config_path = workspace.join("config.json");
        fs::write(
            &provider_config_path,
            json!({
                "model": "mock/mock-provider-model",
                "provider": {
                    "mock": {
                        "api_key": "test-key",
                        "endpoint": "openai_chat_completions",
                        "options": {
                            "base_url": mock_provider.base_url,
                            "timeout_ms": 120000
                        },
                        "models": {
                            "mock-provider-model": {
                                "name": "wire-mock-model",
                                "options": {
                                    "reasoningEffort": "high",
                                    "reasoningSummary": "auto"
                                },
                                "limit": {
                                    "context": 128000,
                                    "output": 2048
                                },
                                "capabilities": {
                                    "streaming": true,
                                    "tool_calling": true,
                                    "reasoning": true
                                }
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let host = AgentOsHost::in_memory().with_runtime_model_config(
            HostRuntimeModelConfig::Provider(ProviderRuntimeModelConfig {
                model: Some("mock/mock-provider-model".to_string()),
                config_path: Some(provider_config_path),
                max_steps: 16,
                max_tokens: None,
                temperature: Some("0.0".to_string()),
            }),
        );
        let mut server = initialized_server(host.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "run provider configured runtime worker".to_string(),
            },
        );
        let shutdown = host.shutdown().unwrap();
        mock_provider.join();

        assert!(
            shutdown.failed_runtime_workers.is_empty(),
            "{:?}",
            shutdown.failed_runtime_workers
        );
        assert_eq!(
            fs::read_to_string(workspace.join("provider-configured.md")).unwrap(),
            "provider configured host worker\n"
        );
        let state = host.kernel().state_snapshot().unwrap();
        let alias = state
            .model_aliases
            .get("mock/mock-provider-model")
            .expect("registered full model alias");
        assert_eq!(alias.provider_id, "mock");
        assert_eq!(alias.provider_model_name, "wire-mock-model");
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
                                    "command": current_exe,
                                    "mode": "exec",
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
        let provider_config_path = workspace.join("config.json");
        fs::write(
            &provider_config_path,
            json!({
                "model": "mock/mock-provider-model",
                "provider": {
                    "mock": {
                        "api_key": "test-key",
                        "endpoint": "openai_chat_completions",
                        "options": {
                            "base_url": mock_provider.base_url
                        },
                        "models": {
                            "mock-provider-model": {
                                "name": "mock-provider-model",
                                "limit": {
                                    "context": 128000,
                                    "output": 1024
                                },
                                "capabilities": {
                                    "streaming": true,
                                    "tool_calling": true
                                }
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let host = AgentOsHost::in_memory().with_runtime_model_config(
            HostRuntimeModelConfig::Provider(ProviderRuntimeModelConfig {
                model: Some("mock/mock-provider-model".to_string()),
                config_path: Some(provider_config_path),
                max_steps: 16,
                max_tokens: Some(128),
                temperature: Some("0.0".to_string()),
            }),
        );
        let mut server = initialized_server(host.clone());
        let thread_id = start_thread(&mut server, &workspace);

        request(
            &mut server,
            AppRequest::TurnStart {
                client_thread_id: thread_id.clone(),
                input: "capture a nonzero command result".to_string(),
            },
        );
        let shutdown = host.shutdown().unwrap();

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

    fn initialized_server(host: AgentOsHost) -> AppServer<AgentOsHost> {
        let mut server = AppServer::new(host);
        request(&mut server, AppRequest::Initialize);
        server
    }

    fn start_thread(server: &mut AppServer<AgentOsHost>, workspace: &Path) -> String {
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

    fn request(server: &mut AppServer<AgentOsHost>, request: AppRequest) -> Value {
        let response = server.handle_envelope(AppRequestEnvelope {
            protocol: agent_os_sys::app_protocol_version(),
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
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: configured.md\\n+configured host worker\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"configured host worker wrote file through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"configured host worker complete\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"configured host worker wrote file\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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
            Self::start_with_expected_model(None, Vec::new(), responses)
        }

        fn start_expect_model_and_request_snippets(
            model: &str,
            snippets: Vec<&str>,
            responses: Vec<Value>,
        ) -> Self {
            Self::start_with_expected_model(
                Some(model.to_string()),
                snippets.into_iter().map(str::to_string).collect(),
                responses,
            )
        }

        fn start_with_expected_model(
            expected_model: Option<String>,
            expected_request_snippets: Vec<String>,
            responses: Vec<Value>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base_url = format!("http://{}", listener.local_addr().unwrap());
            let handle = std::thread::spawn(move || {
                let mut response_index = 0;
                while response_index < responses.len() {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_http_request(&mut stream);
                    if !request.starts_with("POST ") {
                        write!(
                            stream,
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .unwrap();
                        continue;
                    }
                    if let Some(expected_model) = &expected_model {
                        let expected = format!("\"model\":\"{expected_model}\"");
                        assert!(
                            request.contains(&expected),
                            "provider request did not contain {expected}\n{request}"
                        );
                    }
                    for expected in &expected_request_snippets {
                        assert!(
                            request.contains(expected),
                            "provider request did not contain {expected}\n{request}"
                        );
                    }
                    let response = &responses[response_index];
                    response_index += 1;
                    let mut body = response.to_string();
                    if body.contains("__FIRST_EVIDENCE_ID__") {
                        let evidence_id = first_evidence_id(&request)
                            .unwrap_or_else(|| "evd_unavailable".to_string());
                        body = body.replace("__FIRST_EVIDENCE_ID__", &evidence_id);
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

    fn first_evidence_id(input: &str) -> Option<String> {
        let start = input.find("evd_")?;
        let end = input[start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|offset| start + offset)
            .unwrap_or(input.len());
        Some(input[start..end].to_string())
    }
}
