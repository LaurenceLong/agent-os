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

#[test]
#[ignore = "requires LLM_API_KEY and live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "LLM_OPENAI_BASE_URL",
        "http://model.mify.ai.srv/v1",
        "live-openai-compatible-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "LLM_ANTHROPIC_BASE_URL",
        "http://model.mify.ai.srv/anthropic",
        "live-anthropic-compatible-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_agent_control_unsupported_e2e() {
    run_live_llm_goal_driven_agent_control_unsupported_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "LLM_OPENAI_BASE_URL",
        "http://model.mify.ai.srv/v1",
        "live-openai-compatible-goal-agent-control-unsupported.jsonl",
    );
}

#[test]
#[ignore = "requires LLM_API_KEY and live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_agent_control_unsupported_e2e() {
    run_live_llm_goal_driven_agent_control_unsupported_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "LLM_ANTHROPIC_BASE_URL",
        "http://model.mify.ai.srv/anthropic",
        "live-anthropic-compatible-goal-agent-control-unsupported.jsonl",
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

fn run_live_llm_goal_driven_full_tool_surface_e2e(
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
        "aos-live-goal-full-surface-{}-{}",
        provider.replace('-', "_"),
        new_id("t_")
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("read.txt"), "read me from live full surface\n").unwrap();
    std::fs::write(tmp.join("edit.txt"), "status=old\nkeep=this line\n").unwrap();
    std::fs::write(tmp.join("obsolete.tmp"), "delete this file\n").unwrap();
    let verifier_command = write_full_surface_verifier(&tmp);

    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);
    let _ = std::fs::remove_file(&audit_log_path);

    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "live-e2e".to_string(),
            created_by: "agent-os-thread-live-test".to_string(),
            title: "Live full tool surface".to_string(),
            description: "Exercise every model-visible tool through a live LLM".to_string(),
            acceptance_criteria: vec![
                "workspace changes are verified".to_string(),
                "control-plane actions are observable".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 6,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Live full surface".to_string(),
            description: "Live LLM must exercise every model-visible tool".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![
                EvidenceType::SourceRef,
                EvidenceType::DiffRef,
                EvidenceType::CommandLog,
            ],
            priority: 10,
            risk_level: 6,
        })
        .unwrap();
    let supervisor = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            local_goal: "placeholder".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let resume_target =
        live_child_agent(&kernel, &task.task_id, &supervisor, "resume target", &tmp);
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    let stop_target = live_child_agent(&kernel, &task.task_id, &supervisor, "stop target", &tmp);
    let kill_target = live_child_agent(&kernel, &task.task_id, &supervisor, "kill target", &tmp);
    kernel
        .transition_thread(&kill_target.thread_id, ThreadStatus::Running, None)
        .unwrap();
    let approval_id = approve_live_tool_risk(&kernel, &task, &supervisor);
    let mut request_thread = supervisor.clone();
    request_thread.task.local_goal = format!(
        "Complete a live full tool-surface validation using real tool calls. \
Inspect read.txt with read_file. Write created.txt containing FULL_TOOL_SURFACE_OK followed by one newline. \
Replace the exact text status=old with status=new in edit.txt. Delete obsolete.tmp. \
Run this verifier from the workspace: {verifier_command}. \
Use set_objective to record that the live full tool surface goal is achieved. \
Use update_checklist with one completed item. Use record_evidence for read.txt with an external or source claim. \
Use report_supervisor with a concise progress message. Use post_blackboard on channel test-results, scope goal, section test_result. \
Use ask_human to ask whether there is any extra scope. Use agent_control start for a child worker. \
For existing thread {resume_thread}, use agent_control status, output, set_hook, send, set_timeout, export_trace, and resume. \
For existing thread {stop_thread}, use agent_control stop. For existing thread {kill_thread}, use agent_control kill. \
Do not call delete_session or purge_state in this success run; separate live rejection tests cover them. \
Finish with submit_final after the verifier passes.",
        resume_thread = resume_target.thread_id,
        stop_thread = stop_target.thread_id,
        kill_thread = kill_target.thread_id,
    );

    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(4096)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_full_tool_surface_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "task_goal": request_thread.task.local_goal,
            "resume_thread_id": resume_target.thread_id,
            "stop_thread_id": stop_target.thread_id,
            "kill_thread_id": kill_target.thread_id,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request_thread.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 40;
    config.tool_risk_ceiling = 6;
    config.auto_commit_patch_artifacts = false;
    let report = runtime
        .run_to_completion_with_overrides(
            config,
            RuntimeRunOverrides {
                sandbox_profile_id: Some("sbox_workspace_write".to_string()),
                tool_approval_id: Some(approval_id),
            },
        )
        .unwrap();
    assert!(report.final_submitted);
    assert_all_tool_calls_completed(&report);
    assert_eq!(
        std::fs::read_to_string(tmp.join("created.txt")).unwrap(),
        "FULL_TOOL_SURFACE_OK\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("edit.txt")).unwrap(),
        "status=new\nkeep=this line\n"
    );
    assert!(!tmp.join("obsolete.tmp").exists());
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface",
        &report,
        &[
            "read_file",
            "write_file",
            "replace_text",
            "delete_file",
            "run_command",
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
    assert_agent_control_actions(
        &audit_log_path,
        provider,
        "full_tool_surface",
        &report,
        &[
            "start",
            "status",
            "output",
            "set_hook",
            "send",
            "set_timeout",
            "export_trace",
            "resume",
            "stop",
            "kill",
        ],
    );
    println!(
        "live_goal_full_tool_surface_log={}",
        audit_log_path.display()
    );
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_agent_control_unsupported_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    base_env: &str,
    default_base: &str,
    log_file_name: &str,
) {
    for action in ["delete_session", "purge_state"] {
        run_live_llm_goal_driven_single_unsupported_agent_control_e2e(
            provider,
            api_style,
            base_env,
            default_base,
            log_file_name,
            action,
        );
    }
}

fn run_live_llm_goal_driven_single_unsupported_agent_control_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    base_env: &str,
    default_base: &str,
    log_file_name: &str,
    action: &str,
) {
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .expect("LLM_API_KEY or OPENAI_API_KEY is required for live LLM e2e");
    let model = std::env::var("LLM_MODEL")
        .or_else(|_| std::env::var("AGENT_OS_MODEL"))
        .unwrap_or_else(|_| "tongyi/qwen3.6-plus".to_string());
    let api_base = std::env::var(base_env).unwrap_or_else(|_| default_base.to_string());
    let tmp = std::env::temp_dir().join(format!(
        "aos-live-agent-control-unsupported-{}-{}-{}",
        action,
        provider.replace('-', "_"),
        new_id("t_")
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let audit_log_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-os-audit")
        .join(log_file_name);

    let kernel = Kernel::new();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "live-e2e".to_string(),
            created_by: "agent-os-thread-live-test".to_string(),
            title: format!("Live unsupported agent_control {action}"),
            description: "Exercise unsupported agent_control action through a live LLM".to_string(),
            acceptance_criteria: vec![format!("agent_control {action} is rejected")],
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
            local_goal: "placeholder".to_string(),
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
        "unsupported target",
        &tmp,
    );
    let approval_id = approve_live_tool_risk(&kernel, &task, &supervisor);
    let mut request_thread = supervisor.clone();
    request_thread.task.local_goal = format!(
        "Attempt exactly one supervision action for this live negative e2e: call agent_control with action {action} on thread_id {thread_id}. Do not call any other tools before it.",
        thread_id = target.thread_id,
    );
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(1024)
        .with_audit_log(audit_log_path.clone());
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_agent_control_unsupported_start",
            "provider": provider,
            "api_base": api_base,
            "model": model,
            "workspace": tmp,
            "action": action,
            "target_thread_id": target.thread_id,
            "task_goal": request_thread.task.local_goal,
        }),
    )
    .unwrap();

    let mut runtime = ThreadRuntime::new(kernel.clone(), request_thread.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 4;
    config.tool_risk_ceiling = 6;
    let err = runtime
        .run_to_completion_with_overrides(
            config,
            RuntimeRunOverrides {
                sandbox_profile_id: None,
                tool_approval_id: Some(approval_id),
            },
        )
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Unsupported(_)), "{err:?}");
    let state = kernel.state_snapshot().unwrap();
    assert!(state.tool_invocations.values().any(|invocation| {
        invocation.tool_name == "agent_control"
            && invocation.status == ToolCallStatus::Failed
            && invocation.input.get("action").and_then(Value::as_str) == Some(action)
    }));
    append_jsonl(
        &audit_log_path,
        &json!({
            "type": "live_goal_driven_agent_control_unsupported_summary",
            "provider": provider,
            "action": action,
            "error": err.to_string(),
            "tool_invocations": state.tool_invocations,
            "agent_control_commands": state.agent_control_commands,
        }),
    )
    .unwrap();
    println!(
        "live_goal_agent_control_unsupported_log={}",
        audit_log_path.display()
    );
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
    local_goal: &str,
    workspace: &Path,
) -> AgentControlBlock {
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task_id.to_string(),
            role_profile_id: "role_worker".to_string(),
            owner: supervisor.agent_id.clone(),
            local_goal: local_goal.to_string(),
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
                resource_scopes: Vec::new(),
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
