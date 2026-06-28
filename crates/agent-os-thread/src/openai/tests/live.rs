use super::support::*;
use super::*;

#[test]
#[ignore = "requires LLM_API_KEY and live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "LLM_OPENAI_BASE_URL",
        "http://model.mify.ai.srv/v1",
        "live-openai-compatible-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "LLM_ANTHROPIC_BASE_URL",
        "http://model.mify.ai.srv/anthropic",
        "live-anthropic-compatible-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "LLM_OPENAI_BASE_URL",
        "http://model.mify.ai.srv/v1",
        "live-openai-compatible-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "LLM_ANTHROPIC_BASE_URL",
        "http://model.mify.ai.srv/anthropic",
        "live-anthropic-compatible-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "LLM_OPENAI_BASE_URL",
        "http://model.mify.ai.srv/v1",
        "live-openai-compatible-goal-control-plane.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "LLM_ANTHROPIC_BASE_URL",
        "http://model.mify.ai.srv/anthropic",
        "live-anthropic-compatible-goal-control-plane.jsonl",
    );
}

fn run_live_llm_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    base_env: &str,
    default_base: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .expect("LLM_API_KEY or OPENAI_API_KEY is required for live LLM e2e");
    let model = std::env::var("LLM_MODEL")
        .or_else(|_| std::env::var("AGENT_OS_MODEL"))
        .unwrap_or_else(|_| "tongyi/qwen3.6-plus".to_string());
    let api_base = std::env::var(base_env).unwrap_or_else(|_| default_base.to_string());
    let tmp = std::env::temp_dir().join(format!(
        "aos-live-{}-{}",
        provider.replace('-', "_"),
        new_id("t_")
    ));
    std::fs::create_dir_all(&tmp).unwrap();
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

    let (kernel, request) = make_kernel_request_for_role(
            &tmp,
            "role_worker",
            "Create a workspace file named live_result.txt whose entire content is LIVE_LLM_E2E_OK followed by one newline. Verify the file content and finish with a concise final result.",
            Vec::new(),
        );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base)
        .with_api_style(api_style)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 6;
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
    api_style: LlmApiStyle,
    base_env: &str,
    default_base: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .expect("LLM_API_KEY or OPENAI_API_KEY is required for live LLM e2e");
    let model = std::env::var("LLM_MODEL")
        .or_else(|_| std::env::var("AGENT_OS_MODEL"))
        .unwrap_or_else(|_| "tongyi/qwen3.6-plus".to_string());
    let api_base = std::env::var(base_env).unwrap_or_else(|_| default_base.to_string());
    let tmp = std::env::temp_dir().join(format!(
        "aos-live-goal-workspace-{}-{}",
        provider.replace('-', "_"),
        new_id("t_")
    ));
    std::fs::create_dir_all(&tmp).unwrap();
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
    let verifier_name = if cfg!(windows) {
        std::fs::write(
                tmp.join("verify_goal.cmd"),
                "@echo off\r\nfindstr /C:\"Status: ready\" task.md >nul || exit /b 1\r\nfindstr /C:\"WORKSPACE_GOAL_OK\" live_result.txt >nul || exit /b 1\r\nif exist obsolete.tmp exit /b 1\r\necho WORKSPACE_GOAL_VERIFIED\r\n",
            )
            .unwrap();
        "verify_goal.cmd"
    } else {
        std::fs::write(
                tmp.join("verify_goal.sh"),
                "#!/bin/sh\ngrep -F \"Status: ready\" task.md >/dev/null || exit 1\ngrep -F \"WORKSPACE_GOAL_OK\" live_result.txt >/dev/null || exit 1\n[ ! -e obsolete.tmp ] || exit 1\necho WORKSPACE_GOAL_VERIFIED\n",
            )
            .unwrap();
        "verify_goal.sh"
    };

    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);
    let (kernel, request) = make_kernel_request_for_role(
            &tmp,
            "role_worker",
            &format!(
                "Prepare the workspace for release. Inspect task.md, preserve its existing Keep line, change the single status marker from draft to ready, create live_result.txt containing WORKSPACE_GOAL_OK followed by one newline, remove obsolete.tmp, run the provided verifier script {verifier_name}, and finish with a concise final result."
            ),
            Vec::new(),
        );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
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
            "task_goal": request.thread.task.local_goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 10;
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
        &[
            "read_file",
            "write_file",
            "replace_text",
            "delete_file",
            "run_command",
            "submit_final",
        ],
    );
    println!("live_goal_workspace_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_control_plane_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    base_env: &str,
    default_base: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .expect("LLM_API_KEY or OPENAI_API_KEY is required for live LLM e2e");
    let model = std::env::var("LLM_MODEL")
        .or_else(|_| std::env::var("AGENT_OS_MODEL"))
        .unwrap_or_else(|_| "tongyi/qwen3.6-plus".to_string());
    let api_base = std::env::var(base_env).unwrap_or_else(|_| default_base.to_string());
    let tmp = std::env::temp_dir().join(format!(
        "aos-live-goal-control-{}-{}",
        provider.replace('-', "_"),
        new_id("t_")
    ));
    std::fs::create_dir_all(&tmp).unwrap();
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
            "Coordinate this live task as a supervisor. Inspect coordination_seed.md, refresh the durable task objective to say the live control-plane goal is achieved, mark a one-item checklist complete, save an evidence record for the coordination seed, report progress upward, publish one risk note for the shared team blackboard, ask the human to confirm there is no extra scope, start a child worker with a one-sentence assignment, and finish with a concise final result.",
            Vec::new(),
            Vec::new(),
            vec![EvidenceType::SourceRef],
        );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
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
            "task_goal": request.thread.task.local_goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request.thread.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 12;
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
            "set_objective",
            "update_checklist",
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "agent_control",
            "submit_final",
        ],
    );
    println!("live_goal_control_plane_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn assert_all_tool_calls_completed(report: &RuntimeRunReport) {
    let failed: Vec<_> = report
        .tool_results
        .iter()
        .filter(|record| record.status != ToolCallStatus::Completed)
        .collect();
    assert!(failed.is_empty(), "tool calls did not complete: {failed:?}");
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
