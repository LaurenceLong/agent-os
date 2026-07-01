use crate::{
    DaemonRuntimeModelConfig, ExternalRuntimeModelConfig, KernelDaemon, ProviderRuntimeModelConfig,
};
use agent_os_sys::{AgentOsError, AgentOsResult};
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KerneldArgs {
    pub state_db: PathBuf,
    pub runtime_model_config: Option<DaemonRuntimeModelConfig>,
}

impl KerneldArgs {
    pub fn parse(args: impl IntoIterator<Item = String>) -> AgentOsResult<Self> {
        let mut state_db = None;
        let mut stdio = false;
        let mut model_command = None;
        let mut model_args = Vec::new();
        let mut provider = None;
        let mut provider_config = None;
        let mut max_steps = 16u32;
        let mut max_tokens = None;
        let mut temperature = None;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--stdio" => stdio = true,
                "--state-db" => {
                    let path = args.next().ok_or_else(|| {
                        AgentOsError::Validation("--state-db requires a path".to_string())
                    })?;
                    state_db = Some(PathBuf::from(path));
                }
                "--model-command" => {
                    let path = args.next().ok_or_else(|| {
                        AgentOsError::Validation("--model-command requires a path".to_string())
                    })?;
                    model_command = Some(PathBuf::from(path));
                }
                "--model-arg" => {
                    model_args.push(args.next().ok_or_else(|| {
                        AgentOsError::Validation("--model-arg requires a value".to_string())
                    })?);
                }
                "--provider" => {
                    provider = Some(args.next().ok_or_else(|| {
                        AgentOsError::Validation("--provider requires a name".to_string())
                    })?);
                }
                "--provider-config" => {
                    let path = args.next().ok_or_else(|| {
                        AgentOsError::Validation("--provider-config requires a path".to_string())
                    })?;
                    provider_config = Some(PathBuf::from(path));
                }
                "--max-steps" => {
                    let value = args.next().ok_or_else(|| {
                        AgentOsError::Validation("--max-steps requires a value".to_string())
                    })?;
                    max_steps = value.parse().map_err(|_| {
                        AgentOsError::Validation("--max-steps must be a number".to_string())
                    })?;
                }
                "--max-tokens" => {
                    let value = args.next().ok_or_else(|| {
                        AgentOsError::Validation("--max-tokens requires a value".to_string())
                    })?;
                    max_tokens = Some(value.parse().map_err(|_| {
                        AgentOsError::Validation("--max-tokens must be a number".to_string())
                    })?);
                }
                "--temperature" => {
                    temperature = Some(args.next().ok_or_else(|| {
                        AgentOsError::Validation("--temperature requires a value".to_string())
                    })?);
                }
                _ => {
                    return Err(AgentOsError::Validation(format!(
                        "unknown kerneld argument {arg}"
                    )))
                }
            }
        }
        if !stdio {
            return Err(AgentOsError::Validation(
                "kerneld requires --stdio transport".to_string(),
            ));
        }
        let state_db = state_db
            .ok_or_else(|| AgentOsError::Validation("kerneld requires --state-db".to_string()))?;
        if model_command.is_some() && (provider.is_some() || provider_config.is_some()) {
            return Err(AgentOsError::Validation(
                "--model-command cannot be combined with provider runtime config".to_string(),
            ));
        }
        let runtime_model_config = if let Some(program) = model_command {
            Some(DaemonRuntimeModelConfig::External(
                ExternalRuntimeModelConfig {
                    program,
                    args: model_args,
                    max_steps,
                },
            ))
        } else if provider.is_some()
            || provider_config.is_some()
            || max_tokens.is_some()
            || temperature.is_some()
        {
            Some(DaemonRuntimeModelConfig::Provider(
                ProviderRuntimeModelConfig {
                    provider,
                    config_path: provider_config,
                    max_steps,
                    max_tokens,
                    temperature,
                },
            ))
        } else {
            None
        };
        Ok(Self {
            state_db,
            runtime_model_config,
        })
    }
}

pub fn run_stdio_daemon<I, R, W>(args: I, reader: R, writer: W) -> AgentOsResult<()>
where
    I: IntoIterator<Item = String>,
    R: BufRead,
    W: Write,
{
    let args = KerneldArgs::parse(args)?;
    let daemon = match args.runtime_model_config {
        Some(config) => KernelDaemon::open_sqlite(args.state_db)?.with_runtime_model_config(config),
        None => KernelDaemon::open_sqlite(args.state_db)?,
    };
    let serve_result = daemon.clone().serve_jsonl(reader, writer);
    let shutdown_result = daemon.shutdown();
    serve_result?;
    shutdown_result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_sys::{
        AppRequest, AppRequestEnvelope, AppResponse, ClientConnection, ClientKind, SecurityLevel,
    };
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn stdio_daemon_serves_jsonl_from_state_db() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-os-kerneld-stdio-{}-{unique}.sqlite",
            std::process::id()
        ));
        let input = format!(
            "{}\n",
            serde_json::to_string(&AppRequestEnvelope {
                request_id: "req_init".to_string(),
                client: ClientConnection {
                    client_id: "human_1".to_string(),
                    client_name: "Terminal".to_string(),
                    client_kind: ClientKind::TerminalUi,
                    authority: SecurityLevel::HUMAN_ROOT,
                    connected_at: "2026-06-30T00:00:00Z".to_string(),
                },
                request: AppRequest::Initialize,
            })
            .unwrap()
        );
        let mut output = Vec::new();

        run_stdio_daemon(
            [
                "--stdio".to_string(),
                "--state-db".to_string(),
                path.to_string_lossy().to_string(),
            ],
            Cursor::new(input.as_bytes()),
            &mut output,
        )
        .unwrap();

        assert!(path.exists());
        let line = String::from_utf8(output).unwrap();
        let response: agent_os_sys::AppResponseEnvelope =
            serde_json::from_str(line.trim()).unwrap();
        assert_eq!(response.request_id, "req_init");
        assert!(matches!(response.response, AppResponse::Accepted(_)));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stdio_daemon_autostarts_external_model_worker_from_args() {
        let root = std::env::temp_dir().join(format!(
            "agent-os-kerneld-stdio-worker-{}-{}",
            std::process::id(),
            agent_os_sys::new_id("case_")
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state_db = root.join("state.sqlite");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let thread_id = seed_thread(&state_db, &workspace);
        let model_program = compile_external_model(&root);
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&AppRequestEnvelope {
                request_id: "req_init".to_string(),
                client: ClientConnection {
                    client_id: "human_1".to_string(),
                    client_name: "Terminal".to_string(),
                    client_kind: ClientKind::TerminalUi,
                    authority: SecurityLevel::HUMAN_ROOT,
                    connected_at: "2026-06-30T00:00:00Z".to_string(),
                },
                request: AppRequest::Initialize,
            })
            .unwrap(),
            serde_json::to_string(&AppRequestEnvelope {
                request_id: "req_turn_start".to_string(),
                client: ClientConnection {
                    client_id: "human_1".to_string(),
                    client_name: "Terminal".to_string(),
                    client_kind: ClientKind::TerminalUi,
                    authority: SecurityLevel::HUMAN_ROOT,
                    connected_at: "2026-06-30T00:00:00Z".to_string(),
                },
                request: AppRequest::TurnStart {
                    client_thread_id: thread_id,
                    input: "run stdio configured worker".to_string(),
                },
            })
            .unwrap()
        );
        let mut output = Vec::new();

        run_stdio_daemon(
            [
                "--stdio".to_string(),
                "--state-db".to_string(),
                state_db.to_string_lossy().to_string(),
                "--model-command".to_string(),
                model_program.to_string_lossy().to_string(),
            ],
            Cursor::new(input.as_bytes()),
            &mut output,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(workspace.join("stdio-configured.md")).unwrap(),
            "stdio configured daemon worker\n"
        );
        let lines = String::from_utf8(output).unwrap();
        assert_eq!(lines.lines().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn kerneld_args_parse_provider_runtime_config() {
        let args = KerneldArgs::parse([
            "--stdio".to_string(),
            "--state-db".to_string(),
            "state.sqlite".to_string(),
            "--provider".to_string(),
            "mock".to_string(),
            "--provider-config".to_string(),
            "providers.json".to_string(),
            "--max-steps".to_string(),
            "9".to_string(),
            "--max-tokens".to_string(),
            "128".to_string(),
            "--temperature".to_string(),
            "0.1".to_string(),
        ])
        .unwrap();

        match args.runtime_model_config.unwrap() {
            DaemonRuntimeModelConfig::Provider(config) => {
                assert_eq!(config.provider.as_deref(), Some("mock"));
                assert_eq!(
                    config.config_path.unwrap(),
                    std::path::PathBuf::from("providers.json")
                );
                assert_eq!(config.max_steps, 9);
                assert_eq!(config.max_tokens, Some(128));
                assert_eq!(config.temperature.as_deref(), Some("0.1"));
            }
            other => panic!("unexpected runtime model config: {other:?}"),
        }
    }

    #[test]
    fn kerneld_args_parse_external_model_args() {
        let args = KerneldArgs::parse([
            "--stdio".to_string(),
            "--state-db".to_string(),
            "state.sqlite".to_string(),
            "--model-command".to_string(),
            "model.exe".to_string(),
            "--model-arg".to_string(),
            "--flag".to_string(),
            "--model-arg".to_string(),
            "value".to_string(),
        ])
        .unwrap();

        match args.runtime_model_config.unwrap() {
            DaemonRuntimeModelConfig::External(config) => {
                assert_eq!(config.program, std::path::PathBuf::from("model.exe"));
                assert_eq!(config.args, vec!["--flag".to_string(), "value".to_string()]);
            }
            other => panic!("unexpected runtime model config: {other:?}"),
        }
    }

    fn seed_thread(state_db: &std::path::Path, workspace: &std::path::Path) -> String {
        let kernel = agent_os_kernel::Kernel::with_replayed_store(
            agent_os_store_sqlite::SqliteStore::open(state_db).unwrap(),
        )
        .unwrap();
        let goal = kernel
            .register_goal(agent_os_kernel::RegisterGoalInput {
                namespace: "stdio-test".to_string(),
                created_by: "test".to_string(),
                title: "stdio worker".to_string(),
                description: "stdio worker".to_string(),
                acceptance_criteria: vec!["worker completes".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(agent_os_kernel::SpawnTaskInput {
                goal_id: goal.goal_id,
                parent_task_id: None,
                title: "stdio task".to_string(),
                description: "stdio task".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 1,
                risk_level: 1,
            })
            .unwrap();
        kernel
            .spawn_agent(agent_os_kernel::SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_worker".to_string(),
                owner: "test".to_string(),
                goal: "stdio worker".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![workspace.to_string_lossy().to_string()],
            })
            .unwrap()
            .thread_id
    }

    fn compile_external_model(root: &std::path::Path) -> std::path::PathBuf {
        let source_path = root.join("stdio_model.rs");
        let model_program = root.join(format!("stdio_model{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(
            &source_path,
            r##"
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: stdio-configured.md\\n+stdio configured daemon worker\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"stdio configured worker wrote file through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"stdio configured daemon worker complete\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"stdio configured daemon worker wrote file\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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
        let output = std::process::Command::new("rustc")
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
}
