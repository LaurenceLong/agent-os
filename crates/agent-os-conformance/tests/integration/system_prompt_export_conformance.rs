use crate::common;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const PROMPT_REVIEW_INSTRUCTION: &str =
    "Project prompt review rule: preserve user work.\nProject prompt review rule: cite local evidence.\n";
const PROMPT_REVIEW_SKILL_NAME: &str = "prompt-review-skill";
const PROMPT_REVIEW_SKILL_CONTENT: &str =
    "Use this skill to inspect model-visible prompt context.\nConfirm tools, skills, MCP entries, and message history have review artifacts.\n";
const PROMPT_REVIEW_MCP_TOOL: &str = "mcp__prompt_review__echo";

#[derive(Clone, Copy)]
struct RoleCase {
    role_profile_id: &'static str,
    label: &'static str,
}

struct IndexEntry {
    role_label: &'static str,
    prompt_md: PathBuf,
    provider_audit_logs: Vec<ProviderAuditEntry>,
    tool_names: Vec<String>,
    request_steps: Vec<RequestStepEntry>,
    runtime_result: &'static str,
}

struct ProviderAuditEntry {
    provider_label: &'static str,
    audit_log: PathBuf,
}

struct RequestStepEntry {
    step_index: usize,
    context_md: PathBuf,
    messages_md: PathBuf,
    tools_md: PathBuf,
}

struct ModelVisibleContextMarkdown<'a> {
    context_md: &'a Path,
    prompt_md: &'a Path,
    messages_md: &'a Path,
    tools_md: &'a Path,
    audit_log: &'a Path,
    provider_request: &'a Value,
    provider: ProviderCase,
    role: RoleCase,
    step_number: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProviderCase {
    OpenAiChatCompletions,
    AnthropicMessages,
}

impl ProviderCase {
    fn label(self) -> &'static str {
        match self {
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    fn endpoint(self) -> common::LlmApiStyle {
        match self {
            Self::OpenAiChatCompletions => common::LlmApiStyle::OpenAiChatCompletions,
            Self::AnthropicMessages => common::LlmApiStyle::AnthropicMessages,
        }
    }
}

#[test]
fn runtime_exports_real_system_prompt_review_bundle_for_core_roles() {
    let audit_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit/model-visible-context-review");
    fs::create_dir_all(&audit_dir).unwrap();
    let audit_dir = audit_dir.canonicalize().unwrap();
    clear_prompt_review_dir(&audit_dir);

    let roles = [
        RoleCase {
            role_profile_id: "role_producer",
            label: "producer",
        },
        RoleCase {
            role_profile_id: "role_supervisor",
            label: "supervisor",
        },
        RoleCase {
            role_profile_id: "role_reviewer",
            label: "reviewer",
        },
    ];
    let providers = [
        ProviderCase::OpenAiChatCompletions,
        ProviderCase::AnthropicMessages,
    ];
    let mut index_entries = Vec::new();

    for role in roles {
        let workspace = prompt_export_workspace(role.label);
        let prompt_md = audit_dir.join(format!("runtime-system-prompt-{}.md", role.label));
        let _ = fs::remove_file(&prompt_md);
        let mut canonical_system_prompt: Option<String> = None;
        let mut canonical_tool_names: Option<Vec<String>> = None;
        let mut canonical_request_steps = Vec::new();
        let mut provider_audit_logs = Vec::new();

        for provider in providers {
            let audit_log = audit_dir.join(format!(
                "runtime-provider-request-{}-{}.jsonl",
                role.label,
                provider.label()
            ));
            let _ = fs::remove_file(&audit_log);

            let captured_requests = run_prompt_export_case(role, provider, &workspace, &audit_log);
            assert_eq!(captured_requests.len(), expected_request_count(role));

            let audit_entries = read_jsonl_entries(&audit_log);
            let provider_requests = audit_entries
                .iter()
                .filter(|entry| entry["type"] == "provider_request")
                .collect::<Vec<_>>();
            assert_eq!(provider_requests.len(), expected_request_count(role));
            let first_provider_request = provider_requests[0];
            let system_prompt =
                system_prompt_from_provider_request(first_provider_request, provider);
            let captured_system_prompt =
                system_prompt_from_raw_request(&captured_requests[0], provider);
            assert_eq!(system_prompt, captured_system_prompt);
            if let Some(expected) = &canonical_system_prompt {
                assert_eq!(expected, system_prompt);
            } else {
                canonical_system_prompt = Some(system_prompt.to_string());
            }
            assert!(system_prompt.contains("# Agent-OS Runtime Contract"));
            assert!(system_prompt.contains("## Visible Tool Summary"));
            assert!(system_prompt.contains("## Imported Instructions"));
            assert!(system_prompt.contains("## Available Skills"));
            assert!(system_prompt.contains("## Imported MCP Tools"));
            assert!(system_prompt.contains("Project prompt review rule: preserve user work."));
            assert!(system_prompt.contains("prompt-review-skill: Inspect model-visible context."));
            assert!(system_prompt.contains(PROMPT_REVIEW_MCP_TOOL));
            assert!(!system_prompt.contains("supervise and escalate"));
            if role.label == "producer" {
                assert!(system_prompt.contains("Producer responsibility:"));
                assert!(system_prompt.contains("coordinate with or escalate to the Supervisor"));
                assert!(!system_prompt.contains("When answering a child permission request"));
            }
            if role.label == "reviewer" {
                assert!(system_prompt.contains("Reviewer responsibility:"));
                assert!(system_prompt.contains("producer-equivalent baseline capability"));
                assert!(system_prompt.contains("coordinate with or escalate to the Supervisor"));
                assert!(!system_prompt.contains("When answering a child permission request"));
            }

            let tool_names = provider_tool_names(first_provider_request, provider);
            assert!(tool_names.contains(&"search_files".to_string()));
            assert!(tool_names.contains(&"read_file".to_string()));
            assert!(tool_names.contains(&"read_image".to_string()));
            assert!(tool_names.contains(&"run_command".to_string()));
            assert!(tool_names.contains(&"submit_final".to_string()));
            if role.label == "supervisor" {
                assert!(tool_names.contains(&"set_goal".to_string()));
                assert!(tool_names.contains(&"agent_control".to_string()));
                assert!(system_prompt.contains("- set_goal:"));
                assert!(system_prompt.contains("- agent_control:"));
                assert!(system_prompt.contains("When answering a child permission request"));
                assert!(!system_prompt.contains("If agent_control or set_goal is not visible"));
            } else {
                assert!(!tool_names.contains(&"set_goal".to_string()));
                assert!(!tool_names.contains(&"agent_control".to_string()));
                assert!(!system_prompt.contains("- set_goal:"));
                assert!(!system_prompt.contains("- agent_control:"));
            }

            write_prompt_markdown(
                &prompt_md,
                &audit_log,
                first_provider_request,
                provider,
                role,
                &workspace,
            );
            let markdown = fs::read_to_string(&prompt_md).unwrap();
            assert_eq!(markdown, system_prompt);
            assert!(markdown.contains("# Agent-OS Runtime Contract"));
            assert!(markdown.contains("## Visible Tool Summary"));
            assert!(!markdown.contains("## Conversation Messages"));
            assert!(!markdown.contains("## Model-Visible Tools"));
            assert!(!markdown.contains("Provider Wire JSON"));
            assert!(provider_tools(first_provider_request, provider)
                .iter()
                .filter(|tool| !provider_tool_name(tool, provider).starts_with("mcp__"))
                .all(|tool| provider_tool_description(tool, provider).contains("Examples:")));
            assert!(provider_tools(first_provider_request, provider)
                .iter()
                .any(|tool| {
                    provider_tool_name(tool, provider) == PROMPT_REVIEW_MCP_TOOL
                        && provider_tool_description(tool, provider).contains("Echo one text field")
                }));
            if canonical_tool_names.is_none() {
                canonical_tool_names = Some(tool_names.clone());
            }

            if provider == ProviderCase::OpenAiChatCompletions {
                assert!(!markdown.contains("run_command(program"));
                for (step_index, provider_request) in provider_requests.iter().enumerate() {
                    let step_number = step_index + 1;
                    let messages_md = audit_dir.join(format!(
                        "runtime-messages-{}-step-{step_number:02}.md",
                        role.label
                    ));
                    let tools_md = audit_dir.join(format!(
                        "runtime-tools-{}-step-{step_number:02}.md",
                        role.label
                    ));
                    let context_md = audit_dir.join(format!(
                        "runtime-model-visible-context-{}-step-{step_number:02}.md",
                        role.label
                    ));
                    write_messages_markdown(
                        &messages_md,
                        &prompt_md,
                        provider_request,
                        provider,
                        role,
                        step_number,
                    );
                    write_tools_markdown(&tools_md, provider_request, provider, role, step_number);
                    write_model_visible_context_markdown(ModelVisibleContextMarkdown {
                        context_md: &context_md,
                        prompt_md: &prompt_md,
                        messages_md: &messages_md,
                        tools_md: &tools_md,
                        audit_log: &audit_log,
                        provider_request,
                        provider,
                        role,
                        step_number,
                    });
                    canonical_request_steps.push(RequestStepEntry {
                        step_index: step_number,
                        context_md,
                        messages_md,
                        tools_md,
                    });
                }
            }
            provider_audit_logs.push(ProviderAuditEntry {
                provider_label: provider.label(),
                audit_log: audit_log.clone(),
            });
        }

        let first_tools_markdown =
            fs::read_to_string(&canonical_request_steps[0].tools_md).unwrap();
        assert!(first_tools_markdown.contains("### search_files"));
        assert!(first_tools_markdown.contains("### run_command"));
        assert!(first_tools_markdown.contains("Examples:"));
        assert!(first_tools_markdown.contains(PROMPT_REVIEW_MCP_TOOL));
        let first_context_markdown =
            fs::read_to_string(&canonical_request_steps[0].context_md).unwrap();
        assert!(first_context_markdown.contains("System Prompt"));
        assert!(first_context_markdown.contains("Provider Tool Schemas"));
        let final_messages_markdown =
            fs::read_to_string(&canonical_request_steps[3].messages_md).unwrap();
        assert!(final_messages_markdown.contains("prompt-review-skill"));
        assert!(final_messages_markdown.contains("context-visible"));
        assert!(final_messages_markdown.contains(PROMPT_REVIEW_MCP_TOOL));
        index_entries.push(IndexEntry {
            role_label: role.label,
            prompt_md: prompt_md.clone(),
            provider_audit_logs,
            tool_names: canonical_tool_names.unwrap(),
            request_steps: canonical_request_steps,
            runtime_result: "completed with submit_final",
        });
        let _ = fs::remove_dir_all(workspace);
    }

    let index_md = audit_dir.join("index.md");
    fs::write(&index_md, review_index_markdown(&index_entries)).unwrap();
    let index_markdown = fs::read_to_string(&index_md).unwrap();
    assert!(index_markdown.contains("## Review Files"));
    assert!(index_markdown.contains("## Raw System Prompt Files"));
    assert!(index_markdown.contains("## Model-Visible Request Contexts"));
    assert!(index_markdown.contains("### producer"));
    assert!(index_markdown.contains("### supervisor"));
    assert!(index_markdown.contains("### reviewer"));
    assert!(!index_markdown.contains("You are Agent-OS,"));
    assert!(!index_markdown.contains("# Agent-OS Runtime Contract"));
    assert!(index_markdown.contains("runtime-system-prompt-producer.md"));
    assert!(
        index_markdown.contains("runtime-provider-request-producer-openai_chat_completions.jsonl")
    );
    assert!(index_markdown.contains("runtime-provider-request-producer-anthropic_messages.jsonl"));
    assert!(index_markdown.contains("runtime-model-visible-context-producer-step-01.md"));
    assert!(index_markdown.contains("runtime-tools-supervisor-step-01.md"));
    assert!(!audit_dir
        .join("runtime-system-prompt-producer-openai_chat_completions.md")
        .exists());
    assert!(!audit_dir
        .join("runtime-tools-producer-openai_chat_completions-step-01.md")
        .exists());
    assert!(!audit_dir
        .join("runtime-messages-producer-anthropic_messages-step-01.md")
        .exists());
    assert!(!audit_dir
        .join("runtime-system-prompt-worker-openai_chat_completions.md")
        .exists());
    let markdown_count = fs::read_dir(&audit_dir)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "md")
        })
        .count();
    assert_eq!(markdown_count, 40);
    println!("model_visible_context_review_index={}", index_md.display());
}

fn clear_prompt_review_dir(audit_dir: &Path) {
    for entry in fs::read_dir(audit_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fs::remove_dir_all(path).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
    }
}

fn review_index_markdown(entries: &[IndexEntry]) -> String {
    let mut markdown = String::from(
        "# Agent-OS Model-Visible Context Review Bundle\n\n\
         This file is the review entrypoint. Each runtime-system-prompt file is the raw system prompt string sent to the provider. Provider messages, tool schemas, skills, and MCP-visible context are mirrored into per-request Markdown files and kept in the JSONL audit logs as raw source of truth.\n\n\
         ## Review Files\n\n\
         | Role | Prompt Markdown | Provider Audit Logs | Visible Tools | Runtime Result |\n\
         | --- | --- | --- | --- | --- |\n",
    );

    for entry in entries {
        let prompt_file = entry.prompt_md.file_name().unwrap().to_string_lossy();
        let audit_logs = entry
            .provider_audit_logs
            .iter()
            .map(|audit| {
                let audit_file = audit.audit_log.file_name().unwrap().to_string_lossy();
                format!(
                    "{}: [{}]({})<br>`{}`",
                    audit.provider_label,
                    audit_file,
                    audit_file,
                    display_path(&audit.audit_log)
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        let tools = entry
            .tool_names
            .iter()
            .map(|tool_name| format!("`{tool_name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        markdown.push_str(&format!(
            "| {} | [{}]({})<br>`{}` | {} | {} | {} |\n",
            entry.role_label,
            prompt_file,
            prompt_file,
            display_path(&entry.prompt_md),
            audit_logs,
            tools,
            entry.runtime_result
        ));
    }

    markdown.push_str("\n## Raw System Prompt Files\n\n");
    markdown.push_str(
        "Open these files directly to inspect the unmodified system prompt. The index intentionally does not copy prompt bodies, so the prompt markdown files remain the single raw source for review.\n\n",
    );
    for entry in entries {
        markdown.push_str(&format!(
            "- {}: [{}]({}) (`{}`)\n",
            entry.role_label,
            entry.prompt_md.file_name().unwrap().to_string_lossy(),
            entry.prompt_md.file_name().unwrap().to_string_lossy(),
            display_path(&entry.prompt_md),
        ));
    }
    markdown.push_str("\n## Model-Visible Request Contexts\n\n");
    markdown.push_str(
        "Open the per-step context file to review the model-visible request as a whole. Each context file links to the raw system prompt, the provider messages, and the provider tool schemas for that request step.\n\n",
    );
    for entry in entries {
        markdown.push_str(&format!("### {}\n\n", entry.role_label));
        for step in &entry.request_steps {
            let context_file = step.context_md.file_name().unwrap().to_string_lossy();
            let messages_file = step.messages_md.file_name().unwrap().to_string_lossy();
            let tools_file = step.tools_md.file_name().unwrap().to_string_lossy();
            markdown.push_str(&format!(
                "- Step {:02}: [context]({context_file}) (`{}`), [messages]({messages_file}) (`{}`), [tools]({tools_file}) (`{}`)\n",
                step.step_index,
                display_path(&step.context_md),
                display_path(&step.messages_md),
                display_path(&step.tools_md)
            ));
        }
        markdown.push('\n');
    }
    markdown
}

fn prompt_export_workspace(role: &str) -> PathBuf {
    let workspace = env::temp_dir().join(format!(
        "aos-model-context-export-{}-{}-{}",
        role,
        std::process::id(),
        common::new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join("task.md"),
        "Review this task through the normal Agent-OS runtime prompt path. Load prompt-review-skill and echo context-visible through the prompt-review MCP tool.\n",
    )
    .unwrap();
    fs::write(workspace.join("AGENTS.md"), PROMPT_REVIEW_INSTRUCTION).unwrap();
    let skill_root = workspace.join(".agent-os/skills/prompt-review-skill");
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(skill_root.join("SKILL.md"), PROMPT_REVIEW_SKILL_CONTENT).unwrap();
    workspace
}

fn run_prompt_export_case(
    role: RoleCase,
    provider: ProviderCase,
    workspace: &Path,
    audit_log: &Path,
) -> Vec<Value> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (requests_tx, requests_rx) = mpsc::channel();
    let expected_requests = expected_request_count(role);
    let server = thread::spawn(move || {
        serve_prompt_export_endpoint(listener, provider, expected_requests, requests_tx)
    });

    let kernel = common::Kernel::new();
    import_prompt_review_instruction(&kernel, workspace);
    import_prompt_review_skill(&kernel, workspace);
    import_prompt_review_mcp(&kernel, workspace);
    let goal = kernel
        .register_goal(common::RegisterGoalInput {
            namespace: "prompt-review".to_string(),
            created_by: "conformance".to_string(),
            title: format!(
                "Export {} {} runtime system prompt",
                role.label,
                provider.label()
            ),
            description: "Run a minimal task and export the generated model-visible context"
                .to_string(),
            acceptance_criteria: vec![
                "system prompt markdown artifact is written".to_string(),
                "provider messages and tool schemas have markdown artifacts".to_string(),
                "runtime task reaches final submission".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 2,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(common::SpawnTaskInput {
            goal_id: goal.goal_id,
            parent_task_id: None,
            title: "Prompt export".to_string(),
            description: "Capture the prompt from the provider request audit log".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![common::EvidenceType::SourceRef],
            priority: 10,
            risk_level: 2,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(common::SpawnAgentInput {
            task_id: task.task_id,
            role_profile_id: role.role_profile_id.to_string(),
            owner: "conformance".to_string(),
            goal: "Read task.md, load prompt-review-skill, call the prompt-review MCP echo tool, then submit a concise final result.".to_string(),
            success_criteria: vec![
                "task.md was read and cited".to_string(),
                "prompt-review-skill was loaded".to_string(),
                "prompt-review MCP echo was called".to_string(),
            ],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();

    let client = agent_os_thread::OpenAiModelClient::new("test-key", "prompt-export-model")
        .with_api_base(endpoint)
        .with_endpoint(provider.endpoint())
        .with_request_timeout(Duration::from_secs(5))
        .with_audit_log(audit_log);
    let mut runtime = agent_os_thread::ThreadRuntime::new(kernel, agent.thread_id, client);
    let mut config = agent_os_thread::RuntimeConfig::workspace_write(workspace);
    config.max_steps = 6;
    config.tool_risk_ceiling = 4;
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report
        .tool_results
        .iter()
        .any(|result| result.tool_name == "read_file"));
    assert!(report
        .tool_results
        .iter()
        .any(|result| result.tool_name == "load_skill"));
    assert!(report
        .tool_results
        .iter()
        .any(|result| result.tool_name == PROMPT_REVIEW_MCP_TOOL));
    assert_eq!(report.status, common::ThreadStatus::Completed);
    assert!(report.final_submitted);
    assert!(report
        .tool_results
        .iter()
        .any(|result| result.tool_name == "submit_final"));

    let captured_requests = requests_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    server.join().unwrap();
    captured_requests
}

fn expected_request_count(_role: RoleCase) -> usize {
    4
}

fn import_prompt_review_instruction(kernel: &common::Kernel, workspace: &Path) {
    kernel
        .import_instruction_document(common::InstructionDocument {
            instruction_id: common::new_id("ins_"),
            source: common::EcosystemSource {
                source_kind: common::EcosystemSourceKind::Agents,
                source_scope: common::EcosystemSourceScope::Project,
                source_path: workspace.join("AGENTS.md").to_string_lossy().to_string(),
            },
            precedence_rank: 10,
            content: PROMPT_REVIEW_INSTRUCTION.to_string(),
            content_hash: "sha256:prompt-review-instruction".to_string(),
            created_at: common::now_rfc3339(),
        })
        .unwrap();
}

fn import_prompt_review_skill(kernel: &common::Kernel, workspace: &Path) {
    kernel
        .import_skill_definition(common::SkillDefinition {
            skill_id: common::new_id("skill_"),
            name: PROMPT_REVIEW_SKILL_NAME.to_string(),
            description: "Inspect model-visible context.".to_string(),
            root_path: workspace
                .join(".agent-os/skills/prompt-review-skill")
                .to_string_lossy()
                .to_string(),
            skill_file_path: workspace
                .join(".agent-os/skills/prompt-review-skill/SKILL.md")
                .to_string_lossy()
                .to_string(),
            source: common::EcosystemSource {
                source_kind: common::EcosystemSourceKind::AgentOs,
                source_scope: common::EcosystemSourceScope::Project,
                source_path: workspace
                    .join(".agent-os/skills/prompt-review-skill/SKILL.md")
                    .to_string_lossy()
                    .to_string(),
            },
            content: PROMPT_REVIEW_SKILL_CONTENT.to_string(),
            metadata: BTreeMap::new(),
            content_hash: "sha256:prompt-review-skill".to_string(),
            created_at: common::now_rfc3339(),
        })
        .unwrap();
}

fn import_prompt_review_mcp(kernel: &common::Kernel, workspace: &Path) {
    let binary = compile_mcp_fixture(workspace);
    let now = common::now_rfc3339();
    let source = common::EcosystemSource {
        source_kind: common::EcosystemSourceKind::AgentOs,
        source_scope: common::EcosystemSourceScope::Project,
        source_path: workspace
            .join(".agent-os/config.json")
            .to_string_lossy()
            .to_string(),
    };
    let server = common::McpServerSpec {
        server_id: common::new_id("mcp_"),
        name: "prompt-review".to_string(),
        transport: common::McpTransportKind::LocalStdio,
        command: vec![binary.to_string_lossy().to_string()],
        environment: BTreeMap::new(),
        enabled: true,
        timeout_ms: 5000,
        source: source.clone(),
        created_at: now.clone(),
    };
    kernel.register_mcp_server_spec(server.clone()).unwrap();
    let input_schema = json!({
        "type": "object",
        "required": ["text"],
        "properties": {"text": {"type": "string"}},
        "additionalProperties": false
    });
    let output_schema = json!({"type": "object"});
    let mut descriptor = common::mcp_tool_descriptor(
        &server,
        "echo",
        "Echo one text field for model-visible context export review.",
        input_schema.clone(),
        output_schema.clone(),
        &now,
    )
    .unwrap();
    descriptor.examples.push(common::ToolExample {
        description: "Echo a short review marker through the local MCP server.".to_string(),
        parameters: json!({"text": "context-visible"}),
        expected_result: "Returns a text content item with the same value.".to_string(),
    });
    kernel
        .register_mcp_tool_definition(common::McpToolDefinition {
            mcp_tool_id: common::new_id("mcptool_"),
            server_name: server.name.clone(),
            tool_name: "echo".to_string(),
            model_tool_name: descriptor.name.clone(),
            description: descriptor.description.clone(),
            input_schema,
            output_schema,
            source,
            tool_descriptor: descriptor,
            created_at: now,
        })
        .unwrap();
}

fn compile_mcp_fixture(workspace: &Path) -> PathBuf {
    let source = workspace.join("prompt_review_mcp_fixture.rs");
    let binary = workspace.join(format!(
        "prompt_review_mcp_fixture{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
        &source,
        r##"
use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if line.contains("\"method\":\"tools/call\"") {
            let text = line.split("\"text\":\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or("");
            println!(r#"{{"jsonrpc":"2.0","id":2,"result":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#, text);
        } else if line.contains("\"method\":\"initialize\"") {
            println!("{}", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"prompt-review-fixture","version":"0.0.1"}}}"#);
        }
    }
}
"##,
    )
    .unwrap();
    let output = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn serve_prompt_export_endpoint(
    listener: TcpListener,
    provider: ProviderCase,
    expected_requests: usize,
    requests_tx: mpsc::Sender<Vec<Value>>,
) {
    let mut requests = Vec::new();
    for step_index in 0..expected_requests {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = read_http_json(&mut stream);
        let response = match (provider, step_index) {
            (ProviderCase::OpenAiChatCompletions, 0) => openai_read_response(),
            (ProviderCase::OpenAiChatCompletions, 1) => openai_load_skill_response(),
            (ProviderCase::OpenAiChatCompletions, 2) => openai_mcp_response(),
            (ProviderCase::OpenAiChatCompletions, _) => {
                openai_final_response(evidence_refs_from_provider_request(&request, provider))
            }
            (ProviderCase::AnthropicMessages, 0) => anthropic_read_response(),
            (ProviderCase::AnthropicMessages, 1) => anthropic_load_skill_response(),
            (ProviderCase::AnthropicMessages, 2) => anthropic_mcp_response(),
            (ProviderCase::AnthropicMessages, _) => {
                anthropic_final_response(evidence_refs_from_provider_request(&request, provider))
            }
        };
        write_http_json(&mut stream, &response);
        requests.push(request);
    }
    requests_tx.send(requests).unwrap();
}

fn openai_read_response() -> Value {
    json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_read_task",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"task.md\"}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 100, "completion_tokens": 8}
    })
}

fn openai_final_response(evidence_refs: Vec<String>) -> Value {
    assert!(
        !evidence_refs.is_empty(),
        "second provider request must include evidence_ids from read_file"
    );
    json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_submit_final",
                    "type": "function",
                    "function": {
                        "name": "submit_final",
                        "arguments": final_submission_arguments(evidence_refs).to_string()
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 120, "completion_tokens": 12}
    })
}

fn openai_load_skill_response() -> Value {
    json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_load_skill",
                    "type": "function",
                    "function": {
                        "name": "load_skill",
                        "arguments": "{\"name\":\"prompt-review-skill\",\"offset\":1,\"limit\":200}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 110, "completion_tokens": 8}
    })
}

fn openai_mcp_response() -> Value {
    json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_prompt_review_mcp",
                    "type": "function",
                    "function": {
                        "name": PROMPT_REVIEW_MCP_TOOL,
                        "arguments": "{\"text\":\"context-visible\"}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 115, "completion_tokens": 8}
    })
}

fn anthropic_read_response() -> Value {
    json!({
        "id": "msg_read",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": "toolu_read_task",
            "name": "read_file",
            "input": {"path": "task.md"}
        }],
        "usage": {"input_tokens": 100, "output_tokens": 8}
    })
}

fn anthropic_final_response(evidence_refs: Vec<String>) -> Value {
    assert!(
        !evidence_refs.is_empty(),
        "second provider request must include evidence_ids from read_file"
    );
    json!({
        "id": "msg_final",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": "toolu_submit_final",
            "name": "submit_final",
            "input": final_submission_arguments(evidence_refs)
        }],
        "usage": {"input_tokens": 120, "output_tokens": 12}
    })
}

fn anthropic_load_skill_response() -> Value {
    json!({
        "id": "msg_load_skill",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": "toolu_load_skill",
            "name": "load_skill",
            "input": {"name": "prompt-review-skill", "offset": 1, "limit": 200}
        }],
        "usage": {"input_tokens": 110, "output_tokens": 8}
    })
}

fn anthropic_mcp_response() -> Value {
    json!({
        "id": "msg_prompt_review_mcp",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": "toolu_prompt_review_mcp",
            "name": PROMPT_REVIEW_MCP_TOOL,
            "input": {"text": "context-visible"}
        }],
        "usage": {"input_tokens": 115, "output_tokens": 8}
    })
}

fn final_submission_arguments(evidence_refs: Vec<String>) -> Value {
    json!({
        "summary": "Exported the runtime model-visible context for review.",
        "evidence_map": [{
            "claim": "task.md was read before final submission",
            "evidence_refs": evidence_refs
        }],
        "tests_run": ["agent-os model-visible context export integration path"],
        "known_risks": [],
        "tests_not_run": [],
        "changed_artifacts": []
    })
}

fn read_http_json(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let (header_end, content_length) = loop {
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "connection closed before headers");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = find_header_end(&bytes) {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("content-length header");
            break (header_end, content_length);
        }
    };

    let body_start = header_end + 4;
    while bytes.len() < body_start + content_length {
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "connection closed before body");
        bytes.extend_from_slice(&buffer[..read]);
    }
    serde_json::from_slice(&bytes[body_start..body_start + content_length]).unwrap()
}

fn write_http_json(stream: &mut TcpStream, body: &Value) {
    let body = serde_json::to_vec(body).unwrap();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(&body).unwrap();
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn evidence_refs_from_provider_request(request: &Value, provider: ProviderCase) -> Vec<String> {
    match provider {
        ProviderCase::OpenAiChatCompletions => request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|message| message["role"] == "tool")
            .filter_map(|message| message["content"].as_str())
            .filter_map(|content| serde_json::from_str::<Value>(content).ok())
            .flat_map(evidence_ids_from_tool_result_content)
            .collect(),
        ProviderCase::AnthropicMessages => request["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|message| message["content"].as_array())
            .flatten()
            .filter(|part| part["type"] == "tool_result")
            .filter_map(|part| part["content"].as_str())
            .filter_map(|content| serde_json::from_str::<Value>(content).ok())
            .flat_map(evidence_ids_from_tool_result_content)
            .collect(),
    }
}

fn evidence_ids_from_tool_result_content(content: Value) -> Vec<String> {
    content["evidence_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn read_jsonl_entries(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn system_prompt_from_provider_request(provider_request: &Value, provider: ProviderCase) -> &str {
    match provider {
        ProviderCase::OpenAiChatCompletions => provider_request["body"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["role"] == "system")
            .and_then(|message| message["content"].as_str())
            .unwrap(),
        ProviderCase::AnthropicMessages => provider_request["body"]["system"].as_str().unwrap(),
    }
}

fn system_prompt_from_raw_request(request: &Value, provider: ProviderCase) -> &str {
    match provider {
        ProviderCase::OpenAiChatCompletions => request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["role"] == "system")
            .and_then(|message| message["content"].as_str())
            .unwrap(),
        ProviderCase::AnthropicMessages => request["system"].as_str().unwrap(),
    }
}

fn provider_tool_names(provider_request: &Value, provider: ProviderCase) -> Vec<String> {
    provider_tools(provider_request, provider)
        .iter()
        .filter_map(|tool| match provider {
            ProviderCase::OpenAiChatCompletions => tool["function"]["name"].as_str(),
            ProviderCase::AnthropicMessages => tool["name"].as_str(),
        })
        .map(str::to_string)
        .collect()
}

fn provider_tool_description(tool: &Value, provider: ProviderCase) -> &str {
    match provider {
        ProviderCase::OpenAiChatCompletions => tool["function"]["description"].as_str().unwrap(),
        ProviderCase::AnthropicMessages => tool["description"].as_str().unwrap(),
    }
}

fn provider_tool_name(tool: &Value, provider: ProviderCase) -> &str {
    match provider {
        ProviderCase::OpenAiChatCompletions => tool["function"]["name"].as_str().unwrap(),
        ProviderCase::AnthropicMessages => tool["name"].as_str().unwrap(),
    }
}

fn provider_tools(provider_request: &Value, _provider: ProviderCase) -> Vec<Value> {
    provider_request["body"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn write_prompt_markdown(
    prompt_md: &Path,
    _audit_log: &Path,
    provider_request: &Value,
    provider: ProviderCase,
    _role: RoleCase,
    _workspace: &Path,
) {
    let prompt = system_prompt_from_provider_request(provider_request, provider);
    fs::write(prompt_md, prompt).unwrap();
}

fn write_messages_markdown(
    messages_md: &Path,
    prompt_md: &Path,
    provider_request: &Value,
    provider: ProviderCase,
    role: RoleCase,
    step_number: usize,
) {
    let mut markdown = format!(
        "# Provider Messages: {} / {} / step {step_number:02}\n\n\
         System prompt: [{}]({})\n\n",
        role.label,
        provider.label(),
        prompt_md.file_name().unwrap().to_string_lossy(),
        prompt_md.file_name().unwrap().to_string_lossy()
    );
    let messages = provider_request["body"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (index, message) in messages.iter().enumerate() {
        let role_name = message["role"].as_str().unwrap_or("unknown");
        markdown.push_str(&format!("## Message {}: {role_name}\n\n", index + 1));
        if role_name == "system" {
            markdown.push_str(
                "The system message is exported in the raw system prompt file linked above.\n\n",
            );
            continue;
        }
        if let Some(name) = message["name"].as_str() {
            markdown.push_str(&format!("- Name: `{name}`\n"));
        }
        if let Some(tool_call_id) = message["tool_call_id"].as_str() {
            markdown.push_str(&format!("- Tool call id: `{tool_call_id}`\n"));
        }
        if message.get("content").is_some() {
            markdown.push_str("\n### Content\n\n");
            append_value_block(&mut markdown, &message["content"]);
        }
        if let Some(tool_calls) = message["tool_calls"].as_array() {
            markdown.push_str("\n### Tool Calls\n\n");
            append_value_block(&mut markdown, &Value::Array(tool_calls.clone()));
        }
        markdown.push('\n');
    }
    fs::write(messages_md, markdown).unwrap();
}

fn write_tools_markdown(
    tools_md: &Path,
    provider_request: &Value,
    provider: ProviderCase,
    role: RoleCase,
    step_number: usize,
) {
    let tools = provider_tools(provider_request, provider);
    let mut markdown = format!(
        "# Provider Tool Schemas: {} / {} / step {step_number:02}\n\n\
         These are the function/tool definitions sent to the provider for this request step. Descriptions include examples when the kernel ToolDescriptor provides them.\n\n",
        role.label,
        provider.label()
    );
    for tool in &tools {
        let name = provider_tool_name(tool, provider);
        let description = provider_tool_description(tool, provider);
        markdown.push_str(&format!("### {name}\n\n"));
        markdown.push_str("Description:\n\n");
        markdown.push_str(description);
        markdown.push_str("\n\nParameters:\n\n");
        append_json_block(&mut markdown, &provider_tool_schema(tool, provider));
        markdown.push('\n');
    }
    fs::write(tools_md, markdown).unwrap();
}

fn write_model_visible_context_markdown(input: ModelVisibleContextMarkdown<'_>) {
    let messages_count = input.provider_request["body"]["messages"]
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    let tools_count = provider_tools(input.provider_request, input.provider).len();
    let mut markdown = format!(
        "# Model-Visible Context: {} / {} / step {:02}\n\n\
         This file is the human-review mirror of one provider request. The raw JSONL audit log remains the source of truth.\n\n\
         - System Prompt: [{}]({})\n\
         - Provider Messages: [{}]({})\n\
         - Provider Tool Schemas: [{}]({})\n\
         - Raw Audit Log: [{}]({})\n\
         - Message count: {messages_count}\n\
         - Tool count: {tools_count}\n\n\
         ## Model-Visible Segments\n\n\
         - The system prompt carries role, workflow, tool-use rules, and imported ecosystem indexes.\n\
         - Provider tool schemas carry the authoritative callable tool names, descriptions, examples, and parameters.\n\
         - Provider messages carry the user task, assistant tool calls, and tool results accumulated by the runtime.\n",
        input.role.label,
        input.provider.label(),
        input.step_number,
        input.prompt_md.file_name().unwrap().to_string_lossy(),
        input.prompt_md.file_name().unwrap().to_string_lossy(),
        input.messages_md.file_name().unwrap().to_string_lossy(),
        input.messages_md.file_name().unwrap().to_string_lossy(),
        input.tools_md.file_name().unwrap().to_string_lossy(),
        input.tools_md.file_name().unwrap().to_string_lossy(),
        input.audit_log.file_name().unwrap().to_string_lossy(),
        input.audit_log.file_name().unwrap().to_string_lossy()
    );
    if system_prompt_from_provider_request(input.provider_request, input.provider)
        .contains("## Available Skills")
    {
        markdown.push_str("- Skills are visible as an index in the system prompt and loadable through `load_skill`.\n");
    }
    if provider_tools(input.provider_request, input.provider)
        .iter()
        .any(|tool| provider_tool_name(tool, input.provider).starts_with("mcp__"))
    {
        markdown.push_str("- MCP tools are visible as dynamic provider tool schemas and listed in the imported MCP prompt section.\n");
    }
    fs::write(input.context_md, markdown).unwrap();
}

fn provider_tool_schema(tool: &Value, provider: ProviderCase) -> Value {
    match provider {
        ProviderCase::OpenAiChatCompletions => tool["function"]["parameters"].clone(),
        ProviderCase::AnthropicMessages => tool["input_schema"].clone(),
    }
}

fn append_value_block(markdown: &mut String, value: &Value) {
    if let Some(text) = value.as_str() {
        if let Ok(json_value) = serde_json::from_str::<Value>(text) {
            append_json_block(markdown, &json_value);
        } else {
            markdown.push_str("`````text\n");
            markdown.push_str(text);
            markdown.push_str("\n`````\n");
        }
    } else {
        append_json_block(markdown, value);
    }
}

fn append_json_block(markdown: &mut String, value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap();
    markdown.push_str("`````json\n");
    markdown.push_str(&rendered);
    markdown.push_str("\n`````\n");
}

fn display_path(path: &Path) -> String {
    let rendered = path.display().to_string();
    rendered
        .strip_prefix(r"\\?\")
        .unwrap_or(&rendered)
        .to_string()
}
