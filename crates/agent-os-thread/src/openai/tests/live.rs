use super::support::*;
use super::*;

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai-compatible-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_e2e_writes_file_and_logs_interaction() {
    run_live_llm_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic-compatible-e2e-interaction.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai-compatible-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_workspace_e2e() {
    run_live_llm_goal_driven_workspace_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic-compatible-goal-workspace.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai-compatible-goal-control-plane.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_control_plane_e2e() {
    run_live_llm_goal_driven_control_plane_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic-compatible-goal-control-plane.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai-compatible-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_full_tool_surface_e2e() {
    run_live_llm_goal_driven_full_tool_surface_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic-compatible-goal-full-tool-surface.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_OPENAI_API_KEY and a live OpenAI-compatible endpoint"]
fn live_openai_compatible_llm_goal_driven_agent_control_lifecycle_success_e2e() {
    run_live_llm_goal_driven_agent_control_lifecycle_success_e2e(
        "openai-compatible",
        LlmApiStyle::OpenAiCompatible,
        "AGENT_OS_LIVE_OPENAI_API_KEY",
        "AGENT_OS_LIVE_OPENAI_MODEL",
        "AGENT_OS_LIVE_OPENAI_BASE_URL",
        "live-openai-compatible-goal-agent-control-lifecycle-success.jsonl",
    );
}

#[test]
#[ignore = "requires AGENT_OS_LIVE_ANTHROPIC_API_KEY and a live Anthropic-compatible endpoint"]
fn live_anthropic_compatible_llm_goal_driven_agent_control_lifecycle_success_e2e() {
    run_live_llm_goal_driven_agent_control_lifecycle_success_e2e(
        "anthropic-compatible",
        LlmApiStyle::AnthropicCompatible,
        "AGENT_OS_LIVE_ANTHROPIC_API_KEY",
        "AGENT_OS_LIVE_ANTHROPIC_MODEL",
        "AGENT_OS_LIVE_ANTHROPIC_BASE_URL",
        "live-anthropic-compatible-goal-agent-control-lifecycle-success.jsonl",
    );
}

fn run_live_llm_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var(api_key_env)
        .unwrap_or_else(|_| panic!("{api_key_env} is required for live LLM e2e"));
    let model = std::env::var(model_env)
        .unwrap_or_else(|_| panic!("{model_env} is required for live LLM e2e"));
    let api_base = std::env::var(base_env)
        .unwrap_or_else(|_| panic!("{base_env} is required for live LLM e2e"));
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
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var(api_key_env)
        .unwrap_or_else(|_| panic!("{api_key_env} is required for live LLM e2e"));
    let model = std::env::var(model_env)
        .unwrap_or_else(|_| panic!("{model_env} is required for live LLM e2e"));
    let api_base = std::env::var(base_env)
        .unwrap_or_else(|_| panic!("{base_env} is required for live LLM e2e"));
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
            "task_goal": request.thread.task.goal,
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
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var(api_key_env)
        .unwrap_or_else(|_| panic!("{api_key_env} is required for live LLM e2e"));
    let model = std::env::var(model_env)
        .unwrap_or_else(|_| panic!("{model_env} is required for live LLM e2e"));
    let api_base = std::env::var(base_env)
        .unwrap_or_else(|_| panic!("{base_env} is required for live LLM e2e"));
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
            "Complete this live control-plane checklist as a supervisor. 1. read_file coordination_seed.md. 2. set_goal with goal saying the live control-plane goal is achieved. 3. update_checklist with one completed item. 4. record_evidence for the coordination seed. 5. report_supervisor with a concise progress message. 6. post_blackboard one task-scoped risk note on the risks channel. 7. ask_human exactly once to confirm there is no extra scope, then continue after delivery. 8. agent_control start a child worker with role_profile_id role_worker and a one-sentence goal in payload.goal. 9. accomplish_goal with a concise summary. 10. submit_final with summary exactly Control-plane coordination complete., tests_run containing read_file coordination_seed.md, and known_risks as an empty array. submit_final must be the last tool call. Do not skip report_supervisor.",
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
            "task_goal": request.thread.task.goal,
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
            "set_goal",
            "update_checklist",
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "agent_control",
            "accomplish_goal",
            "submit_final",
        ],
    );
    println!("live_goal_control_plane_log={}", audit_log_path.display());
    let _ = std::fs::remove_dir_all(tmp);
}

fn run_live_llm_goal_driven_full_tool_surface_e2e(
    provider: &str,
    api_style: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    let api_key = std::env::var(api_key_env)
        .unwrap_or_else(|_| panic!("{api_key_env} is required for live LLM e2e"));
    let model = std::env::var(model_env)
        .unwrap_or_else(|_| panic!("{model_env} is required for live LLM e2e"));
    let api_base = std::env::var(base_env)
        .unwrap_or_else(|_| panic!("{base_env} is required for live LLM e2e"));
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
            "role_worker",
            &format!(
                "Complete this focused workspace validation. Read read.txt, write created.txt with exactly FULL_TOOL_SURFACE_OK followed by one newline, replace status=old with status=new in edit.txt, delete obsolete.tmp, run {verifier_command}, then call accomplish_goal with a concise summary, then submit_final with summary exactly Workspace surface complete., tests_run containing cmd /C verify_full_surface.cmd, and known_risks as an empty array. submit_final must be the last tool call."
            ),
            Vec::new(),
            vec![ArtifactType::Patch],
            vec![EvidenceType::CommandLog],
        );
    let workspace_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut workspace_runtime = ThreadRuntime::new(
        workspace_kernel.clone(),
        workspace_request.thread.thread_id.clone(),
        workspace_client,
    );
    let mut workspace_config = RuntimeConfig::workspace_write(tmp.clone());
    workspace_config.max_steps = 10;
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
    assert_live_goal_tools(
        &audit_log_path,
        provider,
        "full_tool_surface_workspace",
        &workspace_report,
        &[
            "read_file",
            "write_file",
            "replace_text",
            "delete_file",
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
            "Complete this focused control-plane validation. Read coordination_seed.md, set_goal with goal saying the live full-surface control-plane segment is achieved, update_checklist with one completed item, record_evidence for coordination_seed.md as source_ref, report_supervisor with a short progress message, post_blackboard on channel test-results with scope task and section test_result, ask_human exactly once whether there is extra scope and continue after delivery, start one child worker with role_profile_id role_worker and payload.goal, then call accomplish_goal with a concise summary, then submit_final with summary exactly Control-plane surface complete., tests_run containing read_file coordination_seed.md, and known_risks as an empty array. submit_final must be the last tool call.",
            Vec::new(),
            Vec::new(),
            vec![EvidenceType::SourceRef],
        );
    let control_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(2048)
        .with_audit_log(audit_log_path.clone());
    let mut control_runtime = ThreadRuntime::new(
        control_kernel.clone(),
        control_request.thread.thread_id.clone(),
        control_client,
    );
    let mut control_config = RuntimeConfig::workspace_write(tmp.clone());
    control_config.max_steps = 14;
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
            "record_evidence",
            "report_supervisor",
            "post_blackboard",
            "ask_human",
            "agent_control",
            "accomplish_goal",
            "submit_final",
        ],
    );
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
    let lifecycle_kernel = Kernel::new();
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
    let lifecycle_owner = lifecycle_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: lifecycle_target_task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: "prepare focused live agent_control targets".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let resume_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &lifecycle_owner,
        "resume target",
        &tmp,
    );
    lifecycle_kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Ready, None)
        .unwrap();
    lifecycle_kernel
        .transition_thread(&resume_target.thread_id, ThreadStatus::Suspended, None)
        .unwrap();
    let stop_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &lifecycle_owner,
        "stop target",
        &tmp,
    );
    let kill_target = live_child_agent(
        &lifecycle_kernel,
        &lifecycle_target_task.task_id,
        &lifecycle_owner,
        "kill target",
        &tmp,
    );
    lifecycle_kernel
        .transition_thread(&kill_target.thread_id, ThreadStatus::Running, None)
        .unwrap();

    let status_supervisor = lifecycle_kernel
        .spawn_agent(SpawnAgentInput {
            task_id: status_task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: format!(
                "Complete this focused agent_control read-only validation. Read agent_control_seed.md, then for thread_id {} call agent_control status, output, and export_trace exactly once each. Then submit_final with summary exactly Agent control read surface complete. and known_risks as an empty array.",
                resume_target.thread_id
            ),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let status_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut status_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        status_supervisor.thread_id.clone(),
        status_client,
    );
    let mut status_config = RuntimeConfig::workspace_write(tmp.clone());
    status_config.max_steps = 8;
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
            goal: format!(
                "Complete this focused agent_control mutation validation. Read agent_control_seed.md, then for thread_id {} call agent_control set_hook, send, set_timeout, and resume exactly once each. Then submit_final with summary exactly Agent control mutation surface complete. and known_risks as an empty array.",
                resume_target.thread_id
            ),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let mutation_client = OpenAiModelClient::new(api_key.clone(), model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut mutation_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        mutation_supervisor.thread_id.clone(),
        mutation_client,
    );
    let mut mutation_config = RuntimeConfig::workspace_write(tmp.clone());
    mutation_config.max_steps = 10;
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
            goal: format!(
                "Complete this focused agent_control terminal validation. Read agent_control_seed.md, then call agent_control stop exactly once on thread_id {} and agent_control kill exactly once on thread_id {}. Then submit_final with summary exactly Agent control terminal surface complete. and known_risks as an empty array.",
                stop_target.thread_id, kill_target.thread_id
            ),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let terminal_approval_id =
        approve_live_tool_risk(&lifecycle_kernel, &terminal_task, &terminal_supervisor);
    let terminal_client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
        .with_max_tokens(1536)
        .with_audit_log(audit_log_path.clone());
    let mut terminal_runtime = ThreadRuntime::new(
        lifecycle_kernel.clone(),
        terminal_supervisor.thread_id.clone(),
        terminal_client,
    );
    let mut terminal_config = RuntimeConfig::workspace_write(tmp.clone());
    terminal_config.max_steps = 8;
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
    api_style: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
) {
    for action in ["delete_session", "purge_state"] {
        run_live_llm_goal_driven_single_lifecycle_success_agent_control_e2e(
            provider,
            api_style,
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
    api_style: LlmApiStyle,
    api_key_env: &str,
    model_env: &str,
    base_env: &str,
    log_file_name: &str,
    action: &str,
) {
    let api_key = std::env::var(api_key_env)
        .unwrap_or_else(|_| panic!("{api_key_env} is required for live LLM e2e"));
    let model = std::env::var(model_env)
        .unwrap_or_else(|_| panic!("{model_env} is required for live LLM e2e"));
    let api_base = std::env::var(base_env)
        .unwrap_or_else(|_| panic!("{base_env} is required for live LLM e2e"));
    let tmp = std::env::temp_dir().join(format!(
        "aos-live-agent-control-lifecycle-success-{}-{}-{}",
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
    let target_owner = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: "prepare lifecycle success target".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let target = live_child_agent(
        &kernel,
        &task.task_id,
        &target_owner,
        "lifecycle success target",
        &tmp,
    );
    let detailed_goal = format!(
        "Read control_seed.md, then call exactly one agent_control supervision action with action {action} on thread_id {thread_id}. After that action succeeds, call accomplish_goal with a concise summary, then submit_final with summary exactly Agent control lifecycle action applied., tests_run containing read_file control_seed.md, and known_risks as an empty array. submit_final must be the last tool call.",
        thread_id = target.thread_id,
    );
    let supervisor = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_supervisor".to_string(),
            owner: "agent-os-thread-live-test".to_string(),
            goal: detailed_goal,
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![tmp.to_string_lossy().to_string()],
        })
        .unwrap();
    let approval_id = approve_live_tool_risk(&kernel, &task, &supervisor);
    let client = OpenAiModelClient::new(api_key, model.clone())
        .with_api_base(api_base.clone())
        .with_api_style(api_style)
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
        format!("Agent control lifecycle action: {action}\n"),
    )
    .unwrap();
    let mut runtime = ThreadRuntime::new(kernel.clone(), supervisor.thread_id.clone(), client);
    let mut config = RuntimeConfig::workspace_write(tmp.clone());
    config.max_steps = 4;
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
    goal: &str,
    workspace: &Path,
) -> AgentControlBlock {
    kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task_id.to_string(),
            role_profile_id: "role_worker".to_string(),
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
