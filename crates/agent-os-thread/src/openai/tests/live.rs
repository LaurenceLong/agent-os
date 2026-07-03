use super::support::*;
use super::*;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn live_env_var(name: &str) -> String {
    first_present_env_value(std::env::var(name).ok(), live_env_file_values().get(name))
        .unwrap_or_else(|| panic!("{name} is required for live LLM e2e"))
}

fn first_present_env_value(
    process_value: Option<String>,
    file_value: Option<&String>,
) -> Option<String> {
    process_value
        .filter(|value| !value.trim().is_empty())
        .or_else(|| file_value.filter(|value| !value.trim().is_empty()).cloned())
}

fn live_env_file_values() -> &'static BTreeMap<String, String> {
    static LIVE_ENV: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    LIVE_ENV.get_or_init(|| {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(".env");
        let Ok(content) = std::fs::read_to_string(path) else {
            return BTreeMap::new();
        };
        parse_live_env_content(&content)
    })
}

fn parse_live_env_content(content: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches('\u{feff}');
        if key.is_empty() || key.starts_with('#') {
            continue;
        }
        values.insert(key.to_string(), normalize_live_env_value(value.trim()));
    }
    values
}

fn normalize_live_env_value(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn fresh_live_tmp(prefix: &str, provider: &str) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!(
        "{}-{}-{}",
        prefix,
        provider.replace('-', "_"),
        new_id("t_")
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    tmp
}

fn live_kernel_with_blob_stores(workspace: &Path) -> Kernel {
    let artifact_blobs =
        LocalBlobStore::new(workspace.join(".agent-os-blobs").join("artifacts")).unwrap();
    let evidence_blobs =
        LocalBlobStore::new(workspace.join(".agent-os-blobs").join("evidence")).unwrap();
    Kernel::new().with_blob_stores(artifact_blobs, evidence_blobs)
}

fn live_runtime_config(workspace: &Path, max_steps: u32) -> RuntimeConfig {
    let mut config = RuntimeConfig::workspace_write(workspace);
    config.max_steps = max_steps;
    config.fail_on_process_nonzero = false;
    config
}

const LIVE_IMAGE_CAPABLE_MODEL_ALIAS: &str = "live-image-input";
const LIVE_TEXT_ONLY_MODEL_ALIAS: &str = "live-text-only";
const LIVE_IMAGE_OK_MARKER: &str = "READ_IMAGE_LIVE_OK";
const LIVE_IMAGE_UNSUPPORTED_MARKER: &str = "READ_IMAGE_UNSUPPORTED_OK";
const LIVE_IMAGE_PROBE_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFklEQVR42mP4z8BAEmIY1TCqYfhqAACQ+f8B8u7oVwAAAABJRU5ErkJggg==";
const LIVE_IMAGE_PROBE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x91, 0x68,
    0x36, 0x00, 0x00, 0x00, 0x16, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8, 0xcf, 0xc0, 0x40,
    0x12, 0x62, 0x18, 0xd5, 0x30, 0xaa, 0x61, 0xf8, 0x6a, 0x00, 0x00, 0x90, 0xf9, 0xff, 0x01, 0xf2,
    0xee, 0xe8, 0x57, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[test]
fn live_env_file_parser_handles_bom_comments_quotes_and_precedence() {
    let file_values = parse_live_env_content(
        "\u{feff}# comment\nAGENT_OS_LIVE_OPENAI_MODEL=\"gpt-test\"\nEMPTY=\nPLAIN=value\n",
    );

    assert_eq!(
        file_values.get("AGENT_OS_LIVE_OPENAI_MODEL").unwrap(),
        "gpt-test"
    );
    assert_eq!(file_values.get("PLAIN").unwrap(), "value");
    assert_eq!(
        first_present_env_value(Some("from-process".to_string()), file_values.get("PLAIN"))
            .unwrap(),
        "from-process"
    );
    assert_eq!(
        first_present_env_value(Some("   ".to_string()), file_values.get("PLAIN")).unwrap(),
        "value"
    );
    assert!(first_present_env_value(None, file_values.get("EMPTY")).is_none());
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-control-plane.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-control-plane.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_agent_control_lifecycle_success_e2e() {
    run_live_llm_goal_driven_agent_control_lifecycle_success_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-agent-control-lifecycle-success.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_agent_control_lifecycle_success_e2e() {
    run_live_llm_goal_driven_agent_control_lifecycle_success_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-agent-control-lifecycle-success.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_ecosystem_e2e() {
    run_live_llm_goal_driven_ecosystem_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-ecosystem.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_ecosystem_e2e() {
    run_live_llm_goal_driven_ecosystem_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-ecosystem.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live openai_chat_completions endpoint"]
fn live_openai_chat_completions_llm_goal_driven_scoped_context_e2e() {
    run_live_llm_goal_driven_scoped_context_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-goal-scoped-context.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live anthropic_messages endpoint"]
fn live_anthropic_messages_llm_goal_driven_scoped_context_e2e() {
    run_live_llm_goal_driven_scoped_context_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-goal-scoped-context.jsonl",
    );
}

#[test]
#[ignore = "requires a live openai_chat_completions image-capable model"]
fn live_openai_chat_completions_llm_read_image_success_e2e() {
    run_live_llm_read_image_success_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-read-image-success.jsonl",
    );
}

#[test]
#[ignore = "requires a live anthropic_messages image-capable model"]
fn live_anthropic_messages_llm_read_image_success_e2e() {
    run_live_llm_read_image_success_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-read-image-success.jsonl",
    );
}

#[test]
#[ignore = "requires a live openai_chat_completions text-only model"]
fn live_openai_chat_completions_llm_read_image_unsupported_e2e() {
    run_live_llm_read_image_unsupported_e2e(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-read-image-unsupported.jsonl",
    );
}

#[test]
#[ignore = "requires a live anthropic_messages text-only model"]
fn live_anthropic_messages_llm_read_image_unsupported_e2e() {
    run_live_llm_read_image_unsupported_e2e(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-read-image-unsupported.jsonl",
    );
}

#[test]
#[ignore = "requires a live openai_chat_completions text-only model"]
fn live_openai_chat_completions_llm_switches_read_image_context_to_text_only_model() {
    run_live_llm_switch_read_image_context_to_text_only_model(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-read-image-switch-text-only.jsonl",
    );
}

#[test]
#[ignore = "requires a live anthropic_messages text-only model"]
fn live_anthropic_messages_llm_switches_read_image_context_to_text_only_model() {
    run_live_llm_switch_read_image_context_to_text_only_model(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-read-image-switch-text-only.jsonl",
    );
}

#[test]
#[ignore = "requires a live openai_chat_completions text-only model"]
fn live_openai_chat_completions_llm_forced_image_payload_returns_provider_error() {
    run_live_llm_forced_image_payload_returns_provider_error(
        "openai_chat_completions",
        LlmApiStyle::OpenAiChatCompletions,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai_chat_completions-read-image-forced-text-model-error.jsonl",
    );
}

#[test]
#[ignore = "requires a live anthropic_messages text-only model"]
fn live_anthropic_messages_llm_forced_image_payload_returns_provider_error() {
    run_live_llm_forced_image_payload_returns_provider_error(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-read-image-forced-text-model-error.jsonl",
    );
}

#[test]
#[ignore = "requires a live anthropic_messages text-only model with gateway compatibility behavior"]
fn live_anthropic_messages_llm_forced_image_payload_observes_gateway_behavior() {
    run_live_llm_forced_image_payload_observes_gateway_behavior(
        "anthropic_messages",
        LlmApiStyle::AnthropicMessages,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic_messages-read-image-forced-text-model-observed.jsonl",
    );
}

fn run_live_llm_goal_driven_ecosystem_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-goal-ecosystem", provider);
    let skill_name = "live-ecosystem-skill";
    let skill_marker = "LIVE_SKILL_MARKER_SKILL_AND_MCP_E2E";
    let resource_marker = "LIVE_SKILL_RESOURCE_MARKER_E2E";
    let mcp_marker = "LIVE_MCP_MARKER_SKILL_AND_MCP_E2E";
    let kernel = live_kernel_with_blob_stores(&tmp);
    let mcp_tool_name =
        import_live_ecosystem(&kernel, &tmp, skill_name, skill_marker, resource_marker)
            .unwrap_or_else(|error| panic!("import live ecosystem: {error}"));
    let goal = format!(
        "Produce a focused ecosystem evidence report. Call load_skill for skill {skill_name} with offset 3 and limit 2; use that page to find the resource path references/context.txt and the skill marker {skill_marker}. Then call read_skill_resource with name {skill_name}, path references/context.txt, offset 2, and limit 1; use that page to find resource marker {resource_marker}. Then call tool_search with query exactly live echo, call the returned MCP echo tool with text exactly {mcp_marker}, and do not call any MCP tool before tool_search exposes it. Finally submit_final with a summary containing {skill_marker}, {resource_marker}, and {mcp_marker}. The final submit_final call must include an evidence_map that cites evidence_ids from the completed load_skill, read_skill_resource, and MCP tool results."
    );
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let (kernel, request) = make_kernel_request_for_role_on_kernel_with_requirements(
        kernel,
        &tmp,
        "role_producer",
        &goal,
        Vec::new(),
        Vec::new(),
        vec![EvidenceType::SourceRef, EvidenceType::ExternalReference],
    );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_ecosystem_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "skill_name": skill_name,
            "resource_marker": resource_marker,
            "mcp_tool_name": mcp_tool_name,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let config = live_runtime_config(&tmp, 12);
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "ecosystem",
        &report,
        &[
            "load_skill",
            "read_skill_resource",
            "tool_search",
            &mcp_tool_name,
            "submit_final",
        ],
    );
    assert_skill_context_parameters_observed(&report, skill_name, skill_marker, resource_marker);
    assert_ecosystem_final_summary(&report, skill_marker, resource_marker, mcp_marker);
    let mcp_output = report
        .tool_results
        .iter()
        .find(|record| record.tool_name == mcp_tool_name)
        .and_then(|record| record.output.as_ref())
        .and_then(|output| output.pointer("/raw_result/content/0/text"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing MCP echo output for {mcp_tool_name}"));
    assert_eq!(mcp_output, mcp_marker);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_ecosystem_summary",
            "provider": provider,
            "report": report,
            "mcp_output": mcp_output,
        }),
    )
    .unwrap();
    println!("live_goal_ecosystem_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_scoped_context_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-goal-scoped-context", provider);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let verifier_name = if cfg!(windows) {
        "verify_context.cmd"
    } else {
        "verify_context.sh"
    };
    let verifier_command = if cfg!(windows) {
        format!(r#"command ".\{verifier_name}""#)
    } else {
        format!(r#"command "sh {verifier_name}""#)
    };
    let goal = format!(
        "Use the scoped context projection in your prompt to discover the loaded_refs value, the context compaction superseded_refs value, the owner memento title, the thread lifecycle fork source_thread id, and the rollback reason. Create context_result.txt with six lines: CONTEXT_PROJECTION_OK, then the loaded_refs value, then the superseded_refs value, then the owner memento title, then the lifecycle source_thread id, then the rollback reason. Do not write snapshot or compaction ids unless they appear inside those ref values. Verify it by calling run_command with {verifier_command}. Finish with submit_final and cite evidence_ids from completed tool results."
    );
    let kernel = live_kernel_with_blob_stores(&tmp);
    let (kernel, request) = make_kernel_request_for_role_on_kernel_with_requirements(
        kernel,
        &tmp,
        "role_producer",
        &goal,
        Vec::new(),
        vec![ArtifactType::Patch],
        vec![EvidenceType::DiffRef, EvidenceType::CommandLog],
    );
    kernel
        .load_context(agent_os_kernel::LoadContextInput {
            agent_id: request.thread.agent_id.clone(),
            task_id: request.thread.task.task_id.clone(),
            loaded_refs: vec!["prompt-review/context.md".to_string()],
            summary_artifact_id: None,
            freshness: ContextFreshness::Fresh,
            pollution_score: 0.0,
            token_estimate: 512,
        })
        .unwrap();
    kernel
        .compact_context(agent_os_kernel::CompactContextInput {
            thread_id: request.thread.thread_id.clone(),
            agent_id: request.thread.agent_id.clone(),
            task_id: request.thread.task.task_id.clone(),
            summary_artifact_id: None,
            superseded_refs: vec!["context_snapshot:live-obsolete".to_string()],
            token_estimate: 128,
        })
        .unwrap();
    let memento = kernel
        .create_memento(agent_os_kernel::CreateMementoInput {
            owner_agent_id: request.thread.agent_id.clone(),
            owner_thread_id: request.thread.thread_id.clone(),
            goal_id: request.thread.task.goal_id.clone(),
            task_id: request.thread.task.task_id.clone(),
            anchor: MementoAnchor {
                anchor_type: MementoAnchorType::Manual,
                anchor_ref: None,
                condition: None,
            },
            content: MementoContent {
                title: "Review scoped context reminder".to_string(),
                body: "Use this owner reminder after reading scoped context.".to_string(),
                checklist: vec!["cite context evidence".to_string()],
                structured: None,
            },
            projection: MementoProjection {
                mode: MementoProjectionMode::OwnerContext,
                priority: MementoPriority::High,
                max_projection_count: Some(1),
            },
            links: MementoLinks::default(),
            supersedes: None,
            expires_at: None,
        })
        .unwrap();
    kernel
        .arm_memento(&request.thread.agent_id, &memento.memento_id)
        .unwrap();
    let branch_turn = kernel.start_turn(&request.thread.thread_id).unwrap();
    let branch_turn_id = branch_turn.active_turn.turn_id.clone().unwrap();
    kernel
        .fork_thread(agent_os_kernel::ForkThreadInput {
            source_thread_id: request.thread.thread_id.clone(),
            from_turn_id: Some(branch_turn_id.clone()),
            created_by_client_id: "live-scoped-context".to_string(),
            title: Some("Live scoped context branch".to_string()),
            goal: Some("Review forked scoped context".to_string()),
        })
        .unwrap();
    let rollback_reason = "return to scoped context branch";
    kernel
        .rollback_thread(agent_os_kernel::RollbackThreadInput {
            thread_id: request.thread.thread_id.clone(),
            target_turn_id: Some(branch_turn_id),
            target_item_id: None,
            target_event_id: None,
            reason: rollback_reason.to_string(),
            created_by_client_id: "live-scoped-context".to_string(),
        })
        .unwrap();
    if cfg!(windows) {
        std::fs::write(
            tmp.join(verifier_name),
            format!(
                "@echo off\r\nset RESULT=%~dp0context_result.txt\r\nfindstr /C:\"CONTEXT_PROJECTION_OK\" \"%RESULT%\" >nul || exit /b 1\r\nfindstr /C:\"prompt-review/context.md\" \"%RESULT%\" >nul || exit /b 1\r\nfindstr /C:\"context_snapshot:live-obsolete\" \"%RESULT%\" >nul || exit /b 1\r\nfindstr /C:\"Review scoped context reminder\" \"%RESULT%\" >nul || exit /b 1\r\nfindstr /C:\"{}\" \"%RESULT%\" >nul || exit /b 1\r\nfindstr /C:\"{}\" \"%RESULT%\" >nul || exit /b 1\r\necho CONTEXT_PROJECTION_VERIFIED\r\n",
                request.thread.thread_id, rollback_reason
            ),
        )
        .unwrap();
    } else {
        std::fs::write(
            tmp.join(verifier_name),
            format!(
                "#!/bin/sh\ngrep -F \"CONTEXT_PROJECTION_OK\" context_result.txt >/dev/null || exit 1\ngrep -F \"prompt-review/context.md\" context_result.txt >/dev/null || exit 1\ngrep -F \"context_snapshot:live-obsolete\" context_result.txt >/dev/null || exit 1\ngrep -F \"Review scoped context reminder\" context_result.txt >/dev/null || exit 1\ngrep -F \"{}\" context_result.txt >/dev/null || exit 1\ngrep -F \"{}\" context_result.txt >/dev/null || exit 1\necho CONTEXT_PROJECTION_VERIFIED\n",
                request.thread.thread_id, rollback_reason
            ),
        )
        .unwrap();
    }
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_scoped_context_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let config = live_runtime_config(&tmp, 8);
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_completed_tool(&report, "apply_patch");
    assert_completed_tool(&report, "run_command");
    assert_completed_tool(&report, "submit_final");
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "scoped_context",
        &report,
        &["apply_patch", "run_command", "submit_final"],
    );
    let result = std::fs::read_to_string(tmp.join("context_result.txt")).unwrap();
    assert!(result.contains("CONTEXT_PROJECTION_OK"));
    assert!(result.contains("prompt-review/context.md"));
    assert!(result.contains("context_snapshot:live-obsolete"));
    assert!(result.contains("Review scoped context reminder"));
    assert!(result.contains(&request.thread.thread_id));
    assert!(result.contains(rollback_reason));
    assert_provider_request_contains_text(&audit_log_path, "# Scoped Context Projection");
    assert_provider_request_contains_text(&audit_log_path, "prompt-review/context.md");
    assert_provider_request_contains_text(&audit_log_path, "context_snapshot:live-obsolete");
    assert_provider_request_contains_text(&audit_log_path, "Review scoped context reminder");
    assert_provider_request_contains_text(&audit_log_path, "## Thread Lifecycle Context");
    assert_provider_request_contains_text(&audit_log_path, &request.thread.thread_id);
    assert_provider_request_contains_text(&audit_log_path, rollback_reason);
    assert_run_command_succeeded_with_stdout(&report, "CONTEXT_PROJECTION_VERIFIED");
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_scoped_context_summary",
            "provider": provider,
            "report": report,
            "result": result,
        }),
    )
    .unwrap();
    println!("live_goal_scoped_context_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn import_live_ecosystem(
    kernel: &Kernel,
    workspace: &Path,
    skill_name: &str,
    skill_marker: &str,
    resource_marker: &str,
) -> AgentOsResult<String> {
    let now = now_rfc3339();
    let skill_root = workspace.join(".agent-os/skills").join(skill_name);
    let skill_file = skill_root.join("SKILL.md");
    let resource_dir = skill_root.join("references");
    let resource_file = resource_dir.join("context.txt");
    std::fs::create_dir_all(&skill_root)
        .map_err(|error| AgentOsError::Validation(format!("create live skill root: {error}")))?;
    std::fs::create_dir_all(&resource_dir).map_err(|error| {
        AgentOsError::Validation(format!("create live skill resource root: {error}"))
    })?;
    let skill_content = format!(
        "# Live Ecosystem Skill\nThis first page line should be skipped by the live pagination goal.\nPaged skill marker: {skill_marker}\nResource path: references/context.txt\nUse the local MCP echo capability for the MCP marker and cite tool evidence.\n"
    );
    let resource_content = format!(
        "Resource context intro line\nResource page marker: {resource_marker}\nResource trailing line should be skipped by the live pagination goal.\n"
    );
    std::fs::write(&skill_file, &skill_content)
        .map_err(|error| AgentOsError::Validation(format!("write live skill: {error}")))?;
    std::fs::write(&resource_file, &resource_content)
        .map_err(|error| AgentOsError::Validation(format!("write live skill resource: {error}")))?;
    let skill_source = EcosystemSource {
        source_kind: EcosystemSourceKind::AgentOs,
        source_scope: EcosystemSourceScope::Project,
        source_path: skill_file.to_string_lossy().to_string(),
    };
    kernel.import_skill_definition(SkillDefinition {
        skill_id: new_id("skill_"),
        name: skill_name.to_string(),
        description: "Live e2e skill used to prove model-visible skill loading.".to_string(),
        root_path: skill_root.to_string_lossy().to_string(),
        skill_file_path: skill_file.to_string_lossy().to_string(),
        source: skill_source,
        content: skill_content,
        metadata: BTreeMap::new(),
        content_hash: "sha256:live-ecosystem-skill".to_string(),
        created_at: now.clone(),
    })?;

    let mcp_binary = compile_live_mcp_fixture(workspace)?;
    let source = EcosystemSource {
        source_kind: EcosystemSourceKind::AgentOs,
        source_scope: EcosystemSourceScope::Project,
        source_path: workspace
            .join(".agent-os/config.json")
            .to_string_lossy()
            .to_string(),
    };
    let server = McpServerSpec {
        server_id: new_id("mcp_"),
        name: "live-echo".to_string(),
        transport: McpTransportKind::LocalStdio,
        command: vec![mcp_binary.to_string_lossy().to_string()],
        environment: BTreeMap::new(),
        enabled: true,
        timeout_ms: 5000,
        source: source.clone(),
        created_at: now.clone(),
    };
    kernel.register_mcp_server_spec(server.clone())?;
    let input_schema = json!({
        "type": "object",
        "required": ["text"],
        "properties": {"text": {"type": "string"}},
        "additionalProperties": false
    });
    let output_schema = json!({"type": "object"});
    let mut descriptor = agent_os_kernel::mcp_tool_descriptor(
        &server,
        "echo",
        "Echo one text field through a local stdio MCP server for live e2e coverage.",
        input_schema.clone(),
        output_schema.clone(),
        &now,
    )?;
    descriptor.examples.push(ToolExample {
        description: "Echo the live MCP marker.".to_string(),
        parameters: json!({"text": "LIVE_MCP_MARKER_SKILL_AND_MCP_E2E"}),
        expected_result: "Returns a text content item with the same marker.".to_string(),
    });
    let model_tool_name = descriptor.name.clone();
    kernel.register_mcp_tool_definition(McpToolDefinition {
        mcp_tool_id: new_id("mcptool_"),
        server_name: server.name,
        tool_name: "echo".to_string(),
        model_tool_name: model_tool_name.clone(),
        description: descriptor.description.clone(),
        input_schema,
        output_schema,
        source,
        tool_descriptor: descriptor,
        created_at: now,
    })?;
    Ok(model_tool_name)
}

fn compile_live_mcp_fixture(workspace: &Path) -> AgentOsResult<std::path::PathBuf> {
    let source = workspace.join("live_mcp_echo_fixture.rs");
    let binary = workspace.join(format!(
        "live_mcp_echo_fixture{}",
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(
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
            println!("{}", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"live-echo-fixture","version":"0.0.1"}}}"#);
        }
    }
}
"##,
    )
    .map_err(|error| AgentOsError::Validation(format!("write live MCP fixture: {error}")))?;
    let output = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|error| AgentOsError::Validation(format!("compile live MCP fixture: {error}")))?;
    if !output.status.success() {
        return Err(AgentOsError::Validation(format!(
            "compile live MCP fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(binary)
}

fn assert_skill_context_parameters_observed(
    report: &RuntimeRunReport,
    skill_name: &str,
    skill_marker: &str,
    resource_marker: &str,
) {
    let skill_record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "load_skill"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("name"))
                    .and_then(Value::as_str)
                    == Some(skill_name)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("offset"))
                    .and_then(Value::as_u64)
                    == Some(3)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("limit"))
                    .and_then(Value::as_u64)
                    == Some(2)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized load_skill call: {:?}",
                report.tool_results
            )
        });
    let skill_output = skill_record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing load_skill output: {skill_record:?}"));
    assert_eq!(skill_output.get("offset").and_then(Value::as_u64), Some(3));
    assert_eq!(skill_output.get("limit").and_then(Value::as_u64), Some(2));
    assert_eq!(
        skill_output.get("returned_lines").and_then(Value::as_u64),
        Some(2)
    );
    let skill_content = skill_output
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing load_skill content: {skill_record:?}"));
    assert!(skill_content.contains(skill_marker));
    assert!(skill_content.contains("references/context.txt"));
    assert!(!skill_content.contains("# Live Ecosystem Skill"));

    let resource_record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "read_skill_resource"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("name"))
                    .and_then(Value::as_str)
                    == Some(skill_name)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("path"))
                    .and_then(Value::as_str)
                    == Some("references/context.txt")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("offset"))
                    .and_then(Value::as_u64)
                    == Some(2)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("limit"))
                    .and_then(Value::as_u64)
                    == Some(1)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized read_skill_resource call: {:?}",
                report.tool_results
            )
        });
    let resource_output = resource_record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing read_skill_resource output: {resource_record:?}"));
    assert_eq!(
        resource_output.get("offset").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        resource_output.get("limit").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        resource_output
            .get("returned_lines")
            .and_then(Value::as_u64),
        Some(1)
    );
    let resource_content = resource_output
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing read_skill_resource content: {resource_record:?}"));
    assert!(resource_content.contains(resource_marker));
    assert!(!resource_content.contains("Resource context intro line"));
}

fn assert_ecosystem_final_summary(
    report: &RuntimeRunReport,
    skill_marker: &str,
    resource_marker: &str,
    mcp_marker: &str,
) {
    let summary = report
        .tool_results
        .iter()
        .rev()
        .find(|record| record.tool_name == "submit_final")
        .and_then(|record| record.input.as_ref())
        .and_then(|input| input.get("summary"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing submit_final summary"));
    assert!(
        summary.contains(skill_marker),
        "submit_final summary did not contain skill marker: {summary}"
    );
    assert!(
        summary.contains(resource_marker),
        "submit_final summary did not contain resource marker: {summary}"
    );
    assert!(
        summary.contains(mcp_marker),
        "submit_final summary did not contain MCP marker: {summary}"
    );
}

fn run_live_llm_read_image_success_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-read-image-success", provider);
    std::fs::write(tmp.join("image_probe.png"), LIVE_IMAGE_PROBE_PNG).unwrap();
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let goal = format!(
        "Use read_image exactly once for image_probe.png. Do not use read_file or run_command. After read_image completes, call submit_final with summary exactly {LIVE_IMAGE_OK_MARKER}, evidence_map citing the evidence_id from the completed read_image result, tests_run containing read_image image_probe.png, tests_not_run as an empty array, known_risks as an empty array, and unverified_claims as an empty array. submit_final must be the last tool call."
    );
    let (kernel, request) = make_kernel_request_for_role_with_blob_store_and_requirements(
        &tmp,
        "role_producer",
        &goal,
        Vec::new(),
        Vec::new(),
        vec![EvidenceType::SourceRef],
    );
    register_live_model_alias(
        &kernel,
        &request,
        LIVE_IMAGE_CAPABLE_MODEL_ALIAS,
        &model,
        true,
    );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_success_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "model_alias": LIVE_IMAGE_CAPABLE_MODEL_ALIAS,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let mut config = live_runtime_config(&tmp, 8);
    config.requested_model_alias = Some(LIVE_IMAGE_CAPABLE_MODEL_ALIAS.to_string());
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    assert_completed_tool(&report, "read_image");
    assert_submit_final_summary(&report, LIVE_IMAGE_OK_MARKER);
    assert_provider_request_exposes_tool(&audit_log_path, "read_image");
    assert_provider_request_contains_image_payload(&audit_log_path, provider);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_success_summary",
            "provider": provider,
            "report": report,
        }),
    )
    .unwrap();
    println!("live_read_image_success_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_read_image_unsupported_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-read-image-unsupported", provider);
    std::fs::write(tmp.join("image_probe.png"), LIVE_IMAGE_PROBE_PNG).unwrap();
    std::fs::write(
        tmp.join("image_status.txt"),
        "image_input=false\nread_image must be unavailable for this live text-only model.\n",
    )
    .unwrap();
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let goal = format!(
        "This run intentionally uses a model alias without image_input capability. Confirm the unsupported image-input condition by reading image_status.txt. Do not call run_command. Do not attempt image analysis. If read_image is not visible, that is the expected result. Then call submit_final with summary exactly {LIVE_IMAGE_UNSUPPORTED_MARKER}, evidence_map citing the evidence_id from the completed read_file result, tests_run containing read_file image_status.txt, tests_not_run as an empty array, known_risks as an empty array, and unverified_claims as an empty array. submit_final must be the last tool call."
    );
    let (kernel, request) = make_kernel_request_for_role_with_blob_store_and_requirements(
        &tmp,
        "role_producer",
        &goal,
        Vec::new(),
        Vec::new(),
        vec![EvidenceType::SourceRef],
    );
    register_live_model_alias(&kernel, &request, LIVE_TEXT_ONLY_MODEL_ALIAS, &model, false);
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_unsupported_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "model_alias": LIVE_TEXT_ONLY_MODEL_ALIAS,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let mut config = live_runtime_config(&tmp, 8);
    config.requested_model_alias = Some(LIVE_TEXT_ONLY_MODEL_ALIAS.to_string());
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    assert_completed_tool(&report, "read_file");
    assert_submit_final_summary(&report, LIVE_IMAGE_UNSUPPORTED_MARKER);
    assert_no_completed_read_image(&report);
    assert_provider_requests_do_not_expose_tool(&audit_log_path, "read_image");
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_unsupported_summary",
            "provider": provider,
            "report": report,
        }),
    )
    .unwrap();
    println!(
        "live_read_image_unsupported_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_switch_read_image_context_to_text_only_model(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-read-image-switch-text-only", provider);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let request = live_read_image_context_request(
        &tmp,
        false,
        "Continue this conversation after a previous image was read. The current model route is text-only, so do not request image input. Reply normally or call submit_final only if appropriate.",
    );
    let mut client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(512)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_switch_text_only_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "model_capabilities": request.model_capabilities.clone(),
        }),
    )
    .unwrap();

    let response = crate::ModelClient::next(&mut client, &request).unwrap();
    assert!(
        response.actions.iter().all(|action| {
            !matches!(action, ModelAction::ToolCall(tool) if tool.tool_name == "read_image")
        }),
        "text-only switch response attempted read_image: {:?}",
        response.actions
    );
    assert_provider_requests_do_not_expose_tool(&audit_log_path, "read_image");
    assert_provider_requests_do_not_contain_image_payload(&audit_log_path, provider);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_switch_text_only_summary",
            "provider": provider,
            "actions": response.actions,
        }),
    )
    .unwrap();
    println!(
        "live_read_image_switch_text_only_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_forced_image_payload_returns_provider_error(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-read-image-forced-error", provider);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let request = live_read_image_context_request(
        &tmp,
        true,
        "This live negative test intentionally sends a prior read_image image payload to the current provider model. The model is expected to reject image input.",
    );
    let mut client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(512)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_forced_text_model_error_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "model_capabilities": request.model_capabilities.clone(),
        }),
    )
    .unwrap();

    let error = crate::ModelClient::next(&mut client, &request).unwrap_err();
    let error_text = error.to_string();
    assert!(
        error_text.contains("API error") || error_text.contains("image"),
        "forced image payload returned unexpected error: {error_text}"
    );
    assert_provider_request_exposes_tool(&audit_log_path, "read_image");
    assert_provider_request_contains_image_payload(&audit_log_path, provider);
    assert_provider_error_logged(&audit_log_path, provider);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_forced_text_model_error_summary",
            "provider": provider,
            "error": error_text,
        }),
    )
    .unwrap();
    println!(
        "live_read_image_forced_text_model_error_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_forced_image_payload_observes_gateway_behavior(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-read-image-forced-accepted", provider);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let request = live_read_image_context_request(
        &tmp,
        true,
        "This live compatibility test intentionally sends a prior read_image image payload to a provider model that may not support images. Record whether the gateway accepts, ignores, or downgrades the image input.",
    );
    let mut client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(512)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_forced_payload_observed_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "model_capabilities": request.model_capabilities.clone(),
        }),
    )
    .unwrap();

    let outcome = match crate::ModelClient::next(&mut client, &request) {
        Ok(response) => {
            assert_provider_error_not_logged(&audit_log_path, provider);
            json!({
                "status": "accepted",
                "actions": response.actions,
            })
        }
        Err(error) => {
            assert_provider_error_logged(&audit_log_path, provider);
            json!({
                "status": "provider_error",
                "error": error.to_string(),
            })
        }
    };
    assert_provider_request_exposes_tool(&audit_log_path, "read_image");
    assert_provider_request_contains_image_payload(&audit_log_path, provider);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_read_image_forced_payload_observed_summary",
            "provider": provider,
            "outcome": outcome,
        }),
    )
    .unwrap();
    println!(
        "live_read_image_forced_payload_observed_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn live_read_image_context_request(
    workspace: &Path,
    image_input: bool,
    goal: &str,
) -> ModelTurnRequest {
    live_read_image_context_request_with_data_url(
        workspace,
        image_input,
        goal,
        LIVE_IMAGE_PROBE_DATA_URL,
        LIVE_IMAGE_PROBE_PNG.len(),
    )
}

fn live_read_image_context_request_with_data_url(
    workspace: &Path,
    image_input: bool,
    goal: &str,
    data_url: &str,
    bytes_read: usize,
) -> ModelTurnRequest {
    let (_kernel, mut request) = make_kernel_request_for_role_with_blob_store_and_requirements(
        workspace,
        "role_producer",
        goal,
        Vec::new(),
        Vec::new(),
        vec![EvidenceType::SourceRef],
    );
    let mut capabilities = image_capable_model();
    capabilities.image_input = image_input;
    request.model_capabilities = capabilities;
    request.context.tool_results = vec![ToolExecutionRecord {
        call_id: "call_live_image_context".to_string(),
        tool_name: "read_image".to_string(),
        status: ToolCallStatus::Completed,
        input: Some(
            json!({"workspace_root": workspace.to_string_lossy(), "path": "image_probe.png"}),
        ),
        output: Some(json!({
            "tool": "read_image",
            "status": "ok",
            "input": {"workspace_root": workspace.to_string_lossy(), "path": "image_probe.png"},
            "path": "image_probe.png",
            "mime_type": "image/png",
            "encoding": "base64",
            "data_url": data_url,
            "bytes_read": bytes_read
        })),
        evidence_ids: vec!["evd_live_image_context".to_string()],
        evidence_claim: Some("previous image context was read".to_string()),
    }];
    request
}

fn register_live_model_alias(
    kernel: &Kernel,
    request: &ModelTurnRequest,
    alias: &str,
    provider_model_name: &str,
    image_input: bool,
) {
    kernel
        .register_model_alias(
            alias,
            "primary-provider",
            provider_model_name,
            agent_os_sys::ModelCapabilities {
                streaming: true,
                tool_calling: true,
                reasoning: true,
                temperature: true,
                image_input,
                structured_output: true,
            },
            agent_os_sys::ModelLimit {
                context: 128_000,
                input: None,
                output: 4_096,
            },
            &request.thread.config_snapshot.provider_profile_id,
        )
        .unwrap();
}

fn assert_completed_tool(report: &RuntimeRunReport, tool_name: &str) {
    assert!(
        report
            .tool_results
            .iter()
            .any(|record| record.tool_name == tool_name
                && record.status == agent_os_sys::ToolCallStatus::Completed),
        "missing completed tool result for {tool_name}: {:?}",
        report.tool_results
    );
}

fn assert_run_command_succeeded_with_stdout(report: &RuntimeRunReport, expected_stdout: &str) {
    let record = report
        .tool_results
        .iter()
        .rev()
        .find(|record| record.tool_name == "run_command")
        .unwrap_or_else(|| panic!("missing run_command result: {:?}", report.tool_results));
    let output = record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing run_command output: {record:?}"));
    assert_eq!(
        output.get("exit_code").and_then(Value::as_i64),
        Some(0),
        "run_command did not succeed: {record:?}"
    );
    let stdout = output
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing run_command stdout: {record:?}"));
    assert!(
        stdout.contains(expected_stdout),
        "run_command stdout did not contain {expected_stdout}: {stdout}"
    );
}

fn assert_read_file_parameters_observed(report: &RuntimeRunReport) {
    let record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "read_file"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("path"))
                    .and_then(Value::as_str)
                    == Some("paged.txt")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("offset"))
                    .and_then(Value::as_u64)
                    == Some(2)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("limit"))
                    .and_then(Value::as_u64)
                    == Some(2)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized read_file call: {:?}",
                report.tool_results
            )
        });
    let output = record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing read_file output: {record:?}"));
    assert_eq!(output.get("offset").and_then(Value::as_u64), Some(2));
    assert_eq!(output.get("limit").and_then(Value::as_u64), Some(2));
    assert_eq!(
        output.get("returned_lines").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(output.get("next_offset").and_then(Value::as_u64), Some(4));
    let content = output
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing read_file content: {record:?}"));
    assert!(content.contains("page-two"));
    assert!(content.contains("page-three"));
    assert!(!content.contains("page-one"));
}

fn assert_glob_parameters_observed(report: &RuntimeRunReport) {
    let record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "glob_files"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("path"))
                    .and_then(Value::as_str)
                    == Some("notes")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("pattern"))
                    .and_then(Value::as_str)
                    == Some("*.txt")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("offset"))
                    .and_then(Value::as_u64)
                    == Some(1)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("limit"))
                    .and_then(Value::as_u64)
                    == Some(1)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized glob_files call: {:?}",
                report.tool_results
            )
        });
    let output = record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing glob_files output: {record:?}"));
    assert_eq!(output.get("path").and_then(Value::as_str), Some("notes"));
    assert_eq!(output.get("offset").and_then(Value::as_u64), Some(1));
    assert_eq!(output.get("limit").and_then(Value::as_u64), Some(1));
    assert_eq!(
        output.get("returned_matches").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        output.pointer("/matches/0/path").and_then(Value::as_str),
        Some("notes/b.txt")
    );
}

fn assert_grep_parameters_observed(report: &RuntimeRunReport) {
    let record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "grep_files"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("path"))
                    .and_then(Value::as_str)
                    == Some("notes")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("include"))
                    .and_then(Value::as_str)
                    == Some("*.txt")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("pattern"))
                    .and_then(Value::as_str)
                    == Some("Needle")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("case_sensitive"))
                    .and_then(Value::as_bool)
                    == Some(true)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("offset"))
                    .and_then(Value::as_u64)
                    == Some(1)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("limit"))
                    .and_then(Value::as_u64)
                    == Some(1)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized grep_files call: {:?}",
                report.tool_results
            )
        });
    let output = record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing grep_files output: {record:?}"));
    assert_eq!(output.get("path").and_then(Value::as_str), Some("notes"));
    assert_eq!(output.get("include").and_then(Value::as_str), Some("*.txt"));
    assert_eq!(
        output.get("case_sensitive").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(output.get("offset").and_then(Value::as_u64), Some(1));
    assert_eq!(output.get("limit").and_then(Value::as_u64), Some(1));
    assert_eq!(
        output.get("returned_matches").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        output.pointer("/matches/0/path").and_then(Value::as_str),
        Some("notes/b.txt")
    );
    assert_eq!(
        output.pointer("/matches/0/line").and_then(Value::as_str),
        Some("Needle second")
    );
}

fn assert_request_permissions_parameters_observed(report: &RuntimeRunReport) {
    let record = report
        .tool_results
        .iter()
        .find(|record| {
            record.tool_name == "request_permissions"
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("reason"))
                    .and_then(Value::as_str)
                    == Some("Need bounded read_file permission for this control-plane validation.")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.get("scope"))
                    .and_then(Value::as_str)
                    == Some("turn")
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.pointer("/permissions/max_risk_level"))
                    .and_then(Value::as_u64)
                    == Some(2)
                && record
                    .input
                    .as_ref()
                    .and_then(|input| input.pointer("/permissions/approval_required_above"))
                    .and_then(Value::as_u64)
                    == Some(2)
        })
        .unwrap_or_else(|| {
            panic!(
                "missing parameterized request_permissions call: {:?}",
                report.tool_results
            )
        });
    let input = record
        .input
        .as_ref()
        .unwrap_or_else(|| panic!("missing request_permissions input: {record:?}"));
    assert_eq!(
        input.pointer("/permissions/allowed_syscalls"),
        Some(&json!(["tool.invoke"]))
    );
    assert_eq!(
        input.pointer("/permissions/resource_scopes"),
        Some(&json!(["tool:read_file"]))
    );
    assert_eq!(
        input.pointer("/permissions/allowed_tool_names"),
        Some(&json!(["read_file"]))
    );
    assert_eq!(
        input.pointer("/permissions/allowed_tool_driver_classes"),
        Some(&json!(["filesystem"]))
    );
    assert_eq!(
        input.pointer("/permissions/requires_evidence_for"),
        Some(&json!(["read_file"]))
    );
    let output = record
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("missing request_permissions output: {record:?}"));
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("pending")
    );
    assert_eq!(
        output.get("request_status").and_then(Value::as_str),
        Some("Pending")
    );
    assert_eq!(output.get("scope").and_then(Value::as_str), Some("turn"));
    assert!(
        output
            .get("permission_request_id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty()),
        "request_permissions output omitted permission_request_id: {record:?}"
    );
}

fn assert_no_completed_read_image(report: &RuntimeRunReport) {
    let completed_read_image = report.tool_results.iter().any(|record| {
        record.tool_name == "read_image" && record.status == agent_os_sys::ToolCallStatus::Completed
    });
    assert!(
        !completed_read_image,
        "text-only live run completed read_image unexpectedly: {:?}",
        report.tool_results
    );
}

fn assert_submit_final_summary(report: &RuntimeRunReport, expected: &str) {
    let summary = report
        .tool_results
        .iter()
        .rev()
        .find(|record| record.tool_name == "submit_final")
        .and_then(|record| record.input.as_ref())
        .and_then(|input| input.get("summary"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing submit_final summary"));
    assert_eq!(summary, expected);
}

fn assert_provider_request_exposes_tool(audit_log_path: &Path, tool_name: &str) {
    let observed = provider_request_tool_names(audit_log_path);
    assert!(
        observed.iter().any(|name| name == tool_name),
        "provider requests did not expose {tool_name}; observed tools: {observed:?}"
    );
}

fn assert_provider_requests_do_not_expose_tool(audit_log_path: &Path, tool_name: &str) {
    let observed = provider_request_tool_names(audit_log_path);
    assert!(
        observed.iter().all(|name| name != tool_name),
        "provider requests exposed {tool_name} for text-only model; observed tools: {observed:?}"
    );
}

fn assert_provider_request_contains_text(audit_log_path: &Path, expected: &str) {
    let contents = std::fs::read_to_string(audit_log_path).unwrap();
    assert!(
        contents.contains(expected),
        "provider request audit log did not contain {expected:?}: {}",
        audit_log_path.display()
    );
}

fn provider_request_tool_names(audit_log_path: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in read_audit_jsonl(audit_log_path) {
        if entry.get("type").and_then(Value::as_str) != Some("provider_request") {
            continue;
        }
        let Some(tools) = entry.pointer("/body/tools").and_then(Value::as_array) else {
            continue;
        };
        for tool in tools {
            if let Some(name) = tool.pointer("/function/name").and_then(Value::as_str) {
                names.push(name.to_string());
            } else if let Some(name) = tool.get("name").and_then(Value::as_str) {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn assert_provider_request_contains_image_payload(audit_log_path: &Path, provider: &str) {
    let has_image_payload = read_audit_jsonl(audit_log_path).into_iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("provider_request")
            && json_contains_image_payload(entry.pointer("/body/messages").unwrap_or(&Value::Null))
    });
    assert!(
        has_image_payload,
        "{provider} live read_image run did not send an image payload to the provider"
    );
}

fn assert_provider_requests_do_not_contain_image_payload(audit_log_path: &Path, provider: &str) {
    let has_image_payload = read_audit_jsonl(audit_log_path).into_iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("provider_request")
            && json_contains_image_payload(entry.pointer("/body/messages").unwrap_or(&Value::Null))
    });
    assert!(
        !has_image_payload,
        "{provider} text-only switch sent an image payload to the provider"
    );
}

fn assert_provider_error_logged(audit_log_path: &Path, provider: &str) {
    let has_provider_error = read_audit_jsonl(audit_log_path).into_iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("provider_error")
            && entry.get("provider").and_then(Value::as_str) == Some(provider)
    });
    assert!(
        has_provider_error,
        "{provider} forced image payload did not record provider_error"
    );
}

fn assert_provider_error_not_logged(audit_log_path: &Path, provider: &str) {
    let has_provider_error = read_audit_jsonl(audit_log_path).into_iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("provider_error")
            && entry.get("provider").and_then(Value::as_str) == Some(provider)
    });
    assert!(
        !has_provider_error,
        "{provider} compatibility run recorded provider_error unexpectedly"
    );
}

fn json_contains_image_payload(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().any(json_contains_image_payload),
        Value::Object(map) => {
            let is_openai_image = map.get("type").and_then(Value::as_str) == Some("image_url")
                && value
                    .pointer("/image_url/url")
                    .and_then(Value::as_str)
                    .is_some();
            let is_anthropic_image = map.get("type").and_then(Value::as_str) == Some("image")
                && value.pointer("/source/type").and_then(Value::as_str) == Some("base64");
            is_openai_image || is_anthropic_image || map.values().any(json_contains_image_payload)
        }
        _ => false,
    }
}

fn read_audit_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read audit log {}: {error}", path.display()))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}")))
        .collect()
}

fn run_live_llm_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live", provider);
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_e2e_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
        }),
    )
    .unwrap();
    let verifier_command = if cfg!(windows) {
        r#"command "Get-Content live_result.txt""#
    } else {
        r#"command "cat live_result.txt""#
    };
    let goal = format!(
        "Create a workspace file named live_result.txt whose entire content is LIVE_LLM_E2E_OK followed by one newline. Verify the file content by calling run_command with {verifier_command}. Finish with a concise final result. The final submit_final call must include an evidence_map that cites evidence_ids from completed tool results."
    );

    let (kernel, request) = make_kernel_request_for_role_with_blob_store_and_requirements(
        &tmp,
        "role_producer",
        &goal,
        Vec::new(),
        vec![ArtifactType::Patch],
        vec![EvidenceType::DiffRef, EvidenceType::CommandLog],
    );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base)
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let config = live_runtime_config(&tmp, 6);
    let report = runtime.run_to_completion(config).unwrap();
    let result_path = tmp.join("live_result.txt");
    let result = std::fs::read_to_string(&result_path).unwrap();
    assert_eq!(result, "LIVE_LLM_E2E_OK\n");
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "runtime_report",
            "provider": provider,
            "report": report,
            "result_path": result_path,
            "result_content": result,
        }),
    )
    .unwrap();
    println!("live_e2e_interaction_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_workspace_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-goal-workspace", provider);
    std::fs::write(
        tmp.join("task.md"),
        "Title: live workspace goal\nStatus: draft\nKeep: this line must remain\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("obsolete.tmp"),
        "remove this generated scratch file\n",
    )
    .unwrap();
    let (verifier_name, verifier_command) = if cfg!(windows) {
        std::fs::write(
                tmp.join("verify_goal.cmd"),
                "@echo off\r\nfindstr /C:\"Status: ready\" task.md >nul || exit /b 1\r\nfindstr /C:\"WORKSPACE_GOAL_OK\" live_result.txt >nul || exit /b 1\r\nif exist obsolete.tmp exit /b 1\r\necho WORKSPACE_GOAL_VERIFIED\r\n",
            )
            .unwrap();
        ("verify_goal.cmd", r#"command ".\verify_goal.cmd""#)
    } else {
        std::fs::write(
                tmp.join("verify_goal.sh"),
                "#!/bin/sh\ngrep -F \"Status: ready\" task.md >/dev/null || exit 1\ngrep -F \"WORKSPACE_GOAL_OK\" live_result.txt >/dev/null || exit 1\n[ ! -e obsolete.tmp ] || exit 1\necho WORKSPACE_GOAL_VERIFIED\n",
            )
            .unwrap();
        ("verify_goal.sh", r#"command "sh verify_goal.sh""#)
    };

    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let (kernel, request) = make_kernel_request_for_role_with_blob_store_and_requirements(
        &tmp,
        "role_producer",
        &format!(
            "Prepare the workspace for release. Inspect task.md, preserve its existing Keep line, change the single status marker from draft to ready, create live_result.txt containing WORKSPACE_GOAL_OK followed by one newline, remove obsolete.tmp, run the provided verifier script {verifier_name} by calling run_command with {verifier_command}, and finish with a concise final result. The final submit_final call must include an evidence_map that cites evidence_ids from completed tool results."
        ),
        Vec::new(),
        vec![ArtifactType::Patch],
        vec![EvidenceType::DiffRef, EvidenceType::CommandLog],
    );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_workspace_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let config = live_runtime_config(&tmp, 24);
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);

    assert_eq!(
        std::fs::read_to_string(tmp.join("task.md")).unwrap(),
        "Title: live workspace goal\nStatus: ready\nKeep: this line must remain\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("live_result.txt")).unwrap(),
        "WORKSPACE_GOAL_OK\n"
    );
    assert!(!tmp.join("obsolete.tmp").exists());
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "workspace",
        &report,
        &["apply_patch", "read_file", "run_command", "submit_final"],
    );
    println!("live_goal_workspace_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_control_plane_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-goal-control", provider);
    std::fs::write(
            tmp.join("coordination_seed.md"),
            "Coordination seed: live control-plane goal\nRisk channel: risks\nHuman confirmation: needed\n",
        )
        .unwrap();

    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let (kernel, request) = make_kernel_request_for_role_with_blob_store_and_requirements(
            &tmp,
            "role_supervisor",
                "Complete this live control-plane checklist as a supervisor. 1. read_file coordination_seed.md. 2. update_checklist with one completed item. 3. request_permissions with reason exactly Need bounded read_file permission for this control-plane validation., scope turn, and permissions containing max_risk_level 2, allowed_syscalls [tool.invoke], resource_scopes [tool:read_file], allowed_tool_names [read_file], allowed_tool_driver_classes [filesystem], approval_required_above 2, and requires_evidence_for [read_file]. After the permission request returns pending, continue without approving it and without checking its status. 4. record_evidence for the coordination seed. 5. report_supervisor with a concise progress message. 6. post_blackboard one risk note with channel_id exactly risks, scope task, and section risk. 7. ask_human exactly once to confirm there is no extra scope, then continue after delivery. 8. agent_control start exactly once for a child producer with role_profile_id role_producer and a one-sentence goal in payload.goal; do not call agent_control status, output, send, resume, stop, kill, delete_session, purge_state, or export_trace in this checklist. 9. set_goal with target_thread_id set to the child thread_id returned by agent_control start and goal saying the live control-plane goal is achieved; do not set_goal on your own thread. 10. accomplish_goal with a concise summary. 11. submit_final with summary exactly Control-plane coordination complete., evidence_map citing evidence_ids from completed tool results, tests_run containing read_file coordination_seed.md, and known_risks as an empty array. submit_final must be the last tool call. Do not skip request_permissions, ask_human, agent_control start, set_goal, or report_supervisor.",
            Vec::new(),
            Vec::new(),
            vec![EvidenceType::SourceRef],
        );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_control_plane_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "task_goal": request.thread.task.goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let config = live_runtime_config(&tmp, 13);
    let report = runtime.run_to_completion(config).unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "control_plane",
        &report,
        &[
            "read_file",
            "set_goal",
            "update_checklist",
            "request_permissions",
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "agent_control",
            "accomplish_goal",
            "submit_final",
        ],
    );
    assert_request_permissions_parameters_observed(&report);
    println!("live_goal_control_plane_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_full_tool_surface_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp("aos-live-goal-full-surface", provider);
    std::fs::write(tmp.join("read.txt"), "read me from live full surface\n").unwrap();
    std::fs::write(
        tmp.join("paged.txt"),
        "page-one\npage-two\npage-three\npage-four\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join("notes")).unwrap();
    std::fs::write(
        tmp.join("notes").join("a.txt"),
        "Needle first\nneedle lower\n",
    )
    .unwrap();
    std::fs::write(tmp.join("notes").join("b.txt"), "Needle second\n").unwrap();
    std::fs::write(tmp.join("notes").join("c.md"), "Needle markdown\n").unwrap();
    std::fs::write(tmp.join("edit.txt"), "status=old\nkeep=this line\n").unwrap();
    std::fs::write(tmp.join("obsolete.tmp"), "delete this file\n").unwrap();
    let verifier_command = write_full_surface_verifier(&tmp);

    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);

    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_full_tool_surface_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
        }),
    )
    .unwrap();

    let (workspace_kernel, workspace_request) =
        make_kernel_request_for_role_with_blob_store_and_requirements(
            &tmp,
            "role_producer",
            &format!(
                "Complete this focused workspace validation. Use glob_files to locate read.txt by path pattern, use grep_files to confirm read.txt contains read me, then read read.txt. Also exercise workspace tool parameters: call read_file on paged.txt with offset 2 and limit 2; call glob_files with path notes, pattern *.txt, offset 1, and limit 1 so it returns the second txt file; call grep_files with path notes, include *.txt, pattern Needle, case_sensitive true, offset 1, and limit 1 so it returns the second case-sensitive txt match and excludes c.md and lower-case needle. The grep_files result is sufficient; do not read notes/c.md or make extra verification calls for that exclusion. Use apply_patch for every workspace mutation: add created.txt with content exactly FULL_TOOL_SURFACE_OK followed by one newline and no blank second line, update edit.txt by replacing status=old with status=new, and delete obsolete.tmp with an apply_patch delete operation. Use run_command only for the final verifier command {verifier_command}; do not use run_command for listing, deleting, grepping, or editing files. After the verifier succeeds, call accomplish_goal with a concise summary, then submit_final with summary exactly Workspace surface complete., evidence_map citing evidence_ids from completed tool results, tests_run containing {verifier_command}, and known_risks as an empty array. submit_final must be the last tool call."
            ),
            Vec::new(),
            vec![ArtifactType::Patch],
            vec![EvidenceType::CommandLog],
        );
    let workspace_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut workspace_runtime = ThreadRuntime::new(
        workspace_kernel.clone(),
        workspace_request.thread.thread_id.clone(),
        workspace_client,
    );
    let workspace_config = live_runtime_config(&tmp, 20);
    let workspace_report = workspace_runtime
        .run_to_completion(workspace_config)
        .unwrap();
    assert!(workspace_report.final_submitted);
    assert_all_tool_calls_completed(&workspace_report);
    assert_eq!(
        std::fs::read_to_string(tmp.join("created.txt")).unwrap(),
        "FULL_TOOL_SURFACE_OK\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("edit.txt")).unwrap(),
        "status=new\nkeep=this line\n"
    );
    assert!(!tmp.join("obsolete.tmp").exists());
    assert_read_file_parameters_observed(&workspace_report);
    assert_glob_parameters_observed(&workspace_report);
    assert_grep_parameters_observed(&workspace_report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_workspace",
        &workspace_report,
        &[
            "apply_patch",
            "glob_files",
            "grep_files",
            "read_file",
            "run_command",
            "accomplish_goal",
            "submit_final",
        ],
    );

    std::fs::write(
        tmp.join("coordination_seed.md"),
        "Coordination seed: live full surface control-plane segment\n",
    )
    .unwrap();
    let (control_kernel, control_request) =
        make_kernel_request_for_role_with_blob_store_and_requirements(
            &tmp,
            "role_supervisor",
            "Complete this focused control-plane validation. Read coordination_seed.md, update_checklist with one completed item, request_permissions with reason exactly Need bounded read_file permission for this control-plane validation., scope turn, and permissions containing max_risk_level 2, allowed_syscalls [tool.invoke], resource_scopes [tool:read_file], allowed_tool_names [read_file], allowed_tool_driver_classes [filesystem], approval_required_above 2, and requires_evidence_for [read_file]; after it returns pending, continue without approving it and without checking its status. Record_evidence for coordination_seed.md as source_ref, report_supervisor with a short progress message, post_blackboard on channel test-results with scope task and section test_result, ask_human exactly once whether there is extra scope and continue after delivery, agent_control start exactly once for one child producer with role_profile_id role_producer and payload.goal; do not call agent_control status, output, send, resume, stop, kill, delete_session, purge_state, or export_trace in this segment. Then set_goal with target_thread_id set to the child thread_id returned by agent_control start and goal saying the live full-surface control-plane segment is achieved; do not set_goal on your own thread. Then call accomplish_goal with a concise summary, then submit_final with summary exactly Control-plane surface complete., evidence_map citing evidence_ids from completed tool results, tests_run containing read_file coordination_seed.md, and known_risks as an empty array. submit_final must be the last tool call.",
            Vec::new(),
            Vec::new(),
            vec![EvidenceType::SourceRef],
        );
    let control_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut control_runtime = ThreadRuntime::new(
        control_kernel.clone(),
        control_request.thread.thread_id.clone(),
        control_client,
    );
    let control_config = live_runtime_config(&tmp, 15);
    let control_report = control_runtime.run_to_completion(control_config).unwrap();
    assert!(control_report.final_submitted);
    assert_all_tool_calls_completed(&control_report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_control_plane",
        &control_report,
        &[
            "read_file",
            "set_goal",
            "update_checklist",
            "request_permissions",
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "agent_control",
            "accomplish_goal",
            "submit_final",
        ],
    );
    assert_request_permissions_parameters_observed(&control_report);
    assert_agent_control_actions(
        &audit_log_path,
        provider,
        "full_tool_surface_control_plane",
        &control_report,
        &["start"],
    );

    std::fs::write(
        tmp.join("agent_control_seed.md"),
        "Agent control seed: focused live lifecycle validation\n",
    )
    .unwrap();
    let lifecycle_kernel = live_kernel_with_blob_stores(&tmp);
    let lifecycle_goal = lifecycle_kernel
        .register_goal(RegisterGoalInput {
            namespace: "live-e2e".to_string(),
            created_by: "agent-os-thread-live-test".to_string(),
            title: "Live agent control surface".to_string(),
            description: "Exercise agent_control lifecycle actions through focused live LLM goals"
                .to_string(),
            acceptance_criteria: vec!["agent_control lifecycle actions are observable".to_string()],
            constraints: Vec::new(),
            risk_level: 6,
            deadline: None,
        })
        .unwrap();
    let lifecycle_target_task = lifecycle_kernel
        .spawn_task(SpawnTaskInput {
            goal_id: lifecycle_goal.goal_id.clone(),
            parent_task_id: None,
            title: "Live agent control targets".to_string(),
            description: "Prepare target threads for focused agent_control live runs".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let status_task = lifecycle_kernel
        .spawn_task(SpawnTaskInput {
            goal_id: lifecycle_goal.goal_id.clone(),
            parent_task_id: None,
            title: "Live agent control read".to_string(),
            description: "Live LLM must exercise read-only agent_control actions".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let mutation_task = lifecycle_kernel
        .spawn_task(SpawnTaskInput {
            goal_id: lifecycle_goal.goal_id.clone(),
            parent_task_id: None,
            title: "Live agent control mutation".to_string(),
            description: "Live LLM must exercise mutating agent_control actions".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let terminal_task = lifecycle_kernel
        .spawn_task(SpawnTaskInput {
            goal_id: lifecycle_goal.goal_id.clone(),
            parent_task_id: None,
            title: "Live agent control terminal".to_string(),
            description: "Live LLM must exercise terminal agent_control actions".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::SourceRef],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let status_supervisor = lifecycle_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: status_task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: "Complete this focused agent_control read-only validation. Read agent_control_seed.md, then use status_target_thread_id from that file to call agent_control status, output, and export_trace exactly once each. Use the thread_id field only; do not provide agent_id. For output, omit payload.tool_call_id because the seed file does not provide a tool call id. Then submit_final with summary exactly Agent control read surface complete., evidence_map citing evidence_ids from completed tool results, and known_risks as an empty array.".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let status_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &status_supervisor,
        "status target",
        &tmp,
    );
    std::fs::write(
        tmp.join("agent_control_seed.md"),
        format!(
            "Agent control seed: read surface\nstatus_target_thread_id: {}\nUse thread_id only. Do not provide agent_id.\n",
            status_target.thread_id
        ),
    )
    .unwrap();
    let status_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut status_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        status_supervisor.thread_id.clone(),
        status_client,
    );
    let status_config = live_runtime_config(&tmp, 8);
    let status_report = status_runtime.run_to_completion(status_config).unwrap();
    assert!(status_report.final_submitted);
    assert_all_tool_calls_completed(&status_report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_read",
        &status_report,
        &["read_file", "agent_control", "submit_final"],
    );
    assert_agent_control_actions(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_read",
        &status_report,
        &["status", "output", "export_trace"],
    );

    let mutation_supervisor = lifecycle_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: mutation_task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: "Complete this focused agent_control mutation validation. Read agent_control_seed.md, then use mutation_target_thread_id from that file to call agent_control set_hook, send, set_timeout, and resume exactly once each. Use the thread_id field only; do not provide agent_id. For set_hook, include payload.prompt. For send, include payload.message. For set_timeout, include payload.timeout_seconds. Then submit_final with summary exactly Agent control mutation surface complete., evidence_map citing evidence_ids from completed tool results, and known_risks as an empty array.".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let mutation_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &mutation_supervisor,
        "mutation target",
        &tmp,
    );
    lifecycle_kernel
        .transition_thread(&mutation_target.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    lifecycle_kernel
        .transition_thread(&mutation_target.thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    std::fs::write(
        tmp.join("agent_control_seed.md"),
        format!(
            "Agent control seed: mutation surface\nmutation_target_thread_id: {}\nUse thread_id only. Do not provide agent_id.\n",
            mutation_target.thread_id
        ),
    )
    .unwrap();
    let mutation_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut mutation_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        mutation_supervisor.thread_id.clone(),
        mutation_client,
    );
    let mutation_config = live_runtime_config(&tmp, 10);
    let mutation_report = mutation_runtime.run_to_completion(mutation_config).unwrap();
    assert!(mutation_report.final_submitted);
    assert_all_tool_calls_completed(&mutation_report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_mutation",
        &mutation_report,
        &["read_file", "agent_control", "submit_final"],
    );
    assert_agent_control_actions(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_mutation",
        &mutation_report,
        &["set_hook", "send", "set_timeout", "resume"],
    );

    let terminal_supervisor = lifecycle_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: terminal_task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: "Complete this focused agent_control terminal validation. Read agent_control_seed.md, then call agent_control stop exactly once on stop_target_thread_id and agent_control kill exactly once on kill_target_thread_id from that file. Use the thread_id field only; do not provide agent_id. Then submit_final with summary exactly Agent control terminal surface complete., evidence_map citing evidence_ids from completed tool results, and known_risks as an empty array.".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let stop_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &terminal_supervisor,
        "stop target",
        &tmp,
    );
    let kill_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &terminal_supervisor,
        "kill target",
        &tmp,
    );
    lifecycle_kernel
        .transition_thread(&kill_target.thread_id, ThreadStatus::Running, None)
        .unwrap();
    std::fs::write(
        tmp.join("agent_control_seed.md"),
        format!(
            "Agent control seed: terminal surface\nstop_target_thread_id: {}\nkill_target_thread_id: {}\nUse thread_id only. Do not provide agent_id.\n",
            stop_target.thread_id, kill_target.thread_id
        ),
    )
    .unwrap();
    let terminal_approval_id =
        approve_live_tool_risk(&lifecycle_kernel, &terminal_task, &terminal_supervisor);
    let terminal_client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut terminal_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        terminal_supervisor.thread_id.clone(),
        terminal_client,
    );
    let mut terminal_config = live_runtime_config(&tmp, 8);
    terminal_config.tool_risk_ceiling = 6;
    let terminal_report = terminal_runtime
        .run_to_completion_with_overrides(
            terminal_config,
            RuntimeRunOverrides {
                sandbox_profile_id: Some("sbox_workspace_write".to_string()),
                tool_approval_id: Some(terminal_approval_id),
            },
        )
        .unwrap();
    assert!(terminal_report.final_submitted);
    assert_all_tool_calls_completed(&terminal_report);
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_terminal",
        &terminal_report,
        &["read_file", "agent_control", "submit_final"],
    );
    assert_agent_control_actions(
        &audit_log_path,
        provider,
        "full_tool_surface_agent_control_terminal",
        &terminal_report,
        &["stop", "kill"],
    );
    println!(
        "live_goal_full_tool_surface_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_agent_control_lifecycle_success_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    for action in ["delete_session", "purge_state"] {
        run_live_llm_goal_driven_single_lifecycle_success_agent_control_e2e(
            provider,
            endpoint,
            api_key_env,
            model_env,
            base_env,
            log_file_name,
            action,
        );
    }
}

fn run_live_llm_goal_driven_single_lifecycle_success_agent_control_e2e(
    provider: &str,
    endpoint: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
    action: &str,
) {
    let api_key = live_env_var(api_key_env);
    let model = live_env_var(model_env);
    let api_base = live_env_var(base_env);
    let tmp = fresh_live_tmp(
        &format!("aos-live-agent-control-lifecycle-success-{action}"),
        provider,
    );
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);

    let kernel = live_kernel_with_blob_stores(&tmp);
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "live-e2e".to_string(),
            created_by: "agent-os-thread-live-test".to_string(),
            title: format!("Live agent_control lifecycle success {action}"),
            description: "Exercise successful agent_control lifecycle handling through a live LLM"
                .to_string(),
            acceptance_criteria: vec![format!("agent_control {action} is applied")],
            constraints: Vec::new(),
            risk_level: 6,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: format!("Reject {action}"),
            description: format!("Attempt agent_control {action}"),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: Vec::new(),
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let supervisor = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: format!(
                "Read control_seed.md, then call exactly one agent_control supervision action with action {action} on the target thread_id named in that file. Use the thread_id field only; do not provide agent_id. After that action succeeds, call accomplish_goal with a concise summary, then submit_final with summary exactly Agent control lifecycle action applied., evidence_map citing evidence_ids from completed tool results, tests_run containing read_file control_seed.md, and known_risks as an empty array. submit_final must be the last tool call."
            ),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let target = live_child_agent(
        &kernel,
        &task.task_id,
        &supervisor,
        "lifecycle success target",
        &tmp,
    );
    let approval_id = approve_live_tool_risk(&kernel, &task, &supervisor);
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_endpoint(endpoint)
        .with_max_tokens(1024)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_agent_control_lifecycle_success_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "action": action,
            "target_thread_id": target.thread_id,
            "task_goal": supervisor.task.goal,
        }),
    )
    .unwrap();

    std::fs::write(
        tmp.join("control_seed.md"),
        format!(
            "Agent control lifecycle action: {action}\nTarget thread_id: {}\nUse thread_id only. Do not provide agent_id.\n",
            target.thread_id
        ),
    )
    .unwrap();
    let mut runtime = ThreadRuntime::new(kernel.clone(), supervisor.thread_id.clone(), client);
    let mut config = live_runtime_config(&tmp, 6);
    config.tool_risk_ceiling = 6;
    let report = runtime
        .run_to_completion_with_overrides(
            config,
            RuntimeRunOverrides {
                sandbox_profile_id: None,
                tool_approval_id: Some(approval_id),
            },
        )
        .unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    let state = kernel.state_snapshot().unwrap();
    assert!(state.tool_invocations.values().any(|invocation| {
        invocation.tool_name == "agent_control"
            && invocation.status == ToolCallStatus::Completed
            && invocation.input.get("action").and_then(Value::as_str) == Some(action)
    }));
    assert!(state.agent_control_commands.values().any(|command| {
        command.status == AgentControlCommandStatus::Applied
            && command.target_thread_id.as_deref() == Some(&target.thread_id)
    }));
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_agent_control_lifecycle_success_summary",
            "provider": provider,
            "action": action,
            "report": report,
            "tool_invocations": state.tool_invocations,
            "agent_control_commands": state.agent_control_commands,
        }),
    )
    .unwrap();
    println!(
        "live_goal_agent_control_lifecycle_success_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn assert_all_tool_calls_completed(report: &RuntimeRunReport) {
    let failed: Vec<_> = report
        .tool_results
        .iter()
        .filter(|record| {
            record.tool_name != "runtime_feedback" && record.status != ToolCallStatus::Completed
        })
        .collect();
    assert!(failed.is_empty(), "tool calls did not complete: {failed:?}");
}

fn assert_agent_control_actions(
    audit_log_path: &Path,
    provider: &str,
    scenario: &str,
    report: &RuntimeRunReport,
    expected_actions: &[&str],
) {
    let observed_actions: std::collections::BTreeSet<String> = report
        .tool_results
        .iter()
        .filter(|record| record.tool_name == "agent_control")
        .filter_map(|record| record.input.as_ref())
        .filter_map(|input| input.get("action"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let expected: std::collections::BTreeSet<String> = expected_actions
        .iter()
        .map(|action| action.to_string())
        .collect();
    let missing_actions: Vec<String> = expected.difference(&observed_actions).cloned().collect();
    append_jsonl(
        audit_log_path,
        &json!({
            "type": "live_goal_driven_agent_control_action_summary",
            "provider": provider,
            "scenario": scenario,
            "coverage_rate": format!("{}/{}", expected.len() - missing_actions.len(), expected.len()),
            "expected_actions": expected_actions,
            "observed_actions": observed_actions,
            "missing_actions": missing_actions,
        }),
    )
    .unwrap();
    assert!(
        missing_actions.is_empty(),
        "live goal-driven {scenario} e2e missed expected agent_control actions: {missing_actions:?}"
    );
}

fn write_full_surface_verifier(workspace: &Path) -> String {
    if cfg!(windows) {
        std::fs::write(
            workspace.join("verify_full_surface.cmd"),
            "@echo off\r\nfindstr /C:\"FULL_TOOL_SURFACE_OK\" created.txt >nul || exit /b 1\r\nfindstr /C:\"status=new\" edit.txt >nul || exit /b 1\r\nif exist obsolete.tmp exit /b 1\r\necho FULL_TOOL_SURFACE_VERIFIED\r\n",
        )
        .unwrap();
        "cmd /C verify_full_surface.cmd".to_string()
    } else {
        std::fs::write(
            workspace.join("verify_full_surface.sh"),
            "#!/bin/sh\ngrep -F \"FULL_TOOL_SURFACE_OK\" created.txt >/dev/null || exit 1\ngrep -F \"status=new\" edit.txt >/dev/null || exit 1\n[ ! -e obsolete.tmp ] || exit 1\necho FULL_TOOL_SURFACE_VERIFIED\n",
        )
        .unwrap();
        "sh verify_full_surface.sh".to_string()
    }
}

fn live_child_agent(
    kernel: &Kernel,
    task_id: &str,
    supervisor: &AgentControlBlock,
    goal: &str,
    workspace: &Path,
) -> AgentControlBlock {
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task_id.to_string(),
            role_profile_id: "role_producer".to_string(),
            owner: supervisor.agent_id.clone(),
            goal: goal.to_string(),
            success_criteria: vec!["target action is observable".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: Some(supervisor.thread_id.clone()),
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap()
}

fn approve_live_tool_risk(kernel: &Kernel, task: &Task, supervisor: &AgentControlBlock) -> String {
    let approval = kernel
        .request_approval(RequestApprovalInput {
            goal_id: task.goal_id.clone(),
            task_id: Some(task.task_id.clone()),
            requested_by_agent_id: supervisor.agent_id.clone(),
            approval_type: ApprovalType::Human,
            scope: ApprovalScope {
                syscall_types: vec!["tool.invoke".to_string()],
                resource_scopes: vec![json!("tool:*")],
                risk_ceiling: 6,
                goal_id: task.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
            },
            risk_level: 6,
            expires_at: None,
        })
        .unwrap();
    kernel
        .record_approval(RecordApprovalInput {
            approval_id: approval.approval_id.clone(),
            status: ApprovalStatus::Approved,
            decision_by: "agent-os-thread-live-test".to_string(),
            decision_reason: Some("approve bounded live e2e tool coverage".to_string()),
        })
        .unwrap();
    approval.approval_id
}

fn assert_live_goal_tools(
    audit_log_path: &Path,
    provider: &str,
    scenario: &str,
    report: &RuntimeRunReport,
    expected_tools: &[&str],
) {
    let mut observed_tools: std::collections::BTreeSet<String> = report
        .tool_results
        .iter()
        .map(|record| record.tool_name.clone())
        .collect();
    if report.final_submitted {
        observed_tools.insert("submit_final".to_string());
    }
    let expected: std::collections::BTreeSet<String> =
        expected_tools.iter().map(|tool| tool.to_string()).collect();
    let missing_tools: Vec<String> = expected.difference(&observed_tools).cloned().collect();
    append_jsonl(
        audit_log_path,
        &json!({
            "type": "live_goal_driven_summary",
            "provider": provider,
            "scenario": scenario,
            "coverage_rate": format!("{}/{}", expected.len() - missing_tools.len(), expected.len()),
            "expected_tools": expected_tools,
            "observed_tools": observed_tools,
            "missing_tools": missing_tools,
            "tool_results": report.tool_results,
            "final_submitted": report.final_submitted,
        }),
    )
    .unwrap();
    assert!(
        missing_tools.is_empty(),
        "live goal-driven {scenario} e2e missed expected tools: {missing_tools:?}"
    );
}
