use crate::common::*;
use agent_os_store_sqlite::SqliteStore;
use serde_json::Value;
use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_status_and_process_list_binaries_start_hostd_and_read_sqlite_projection() {
    let root = isolated_temp_dir("cli-status-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let state_db = root.join("state").join("agent-os.sqlite");

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("status")
        .arg("--state-db")
        .arg(&state_db)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agent-os status failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["state_db"], state_db.to_string_lossy().to_string());
    assert_eq!(value["threads"].as_array().unwrap().len(), 0);
    assert_eq!(value["stats"]["provider_calls"], 0);
    assert!(state_db.is_file());

    let process_output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("process")
        .arg("list")
        .arg("--state")
        .arg("running")
        .arg("--state-db")
        .arg(&state_db)
        .output()
        .unwrap();

    assert!(
        process_output.status.success(),
        "agent-os process list failed with status {}\nstdout:\n{}\nstderr:\n{}",
        process_output.status,
        String::from_utf8_lossy(&process_output.stdout),
        String::from_utf8_lossy(&process_output.stderr)
    );
    let process_stdout = String::from_utf8(process_output.stdout).unwrap();
    let process_value: Value = serde_json::from_str(&process_stdout).unwrap();
    assert_eq!(
        process_value["state_db"],
        state_db.to_string_lossy().to_string()
    );
    assert_eq!(process_value["action"], "list");
    assert_eq!(
        process_value["process_sessions"].as_array().unwrap().len(),
        0
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_binaries_read_nonempty_sqlite_thread_and_process_projection_through_hostd() {
    let root = isolated_temp_dir("cli-nonempty-sqlite");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let state_db = root.join("state").join("agent-os.sqlite");
    let workspace = root.join("workspace");
    fs::create_dir_all(state_db.parent().unwrap()).unwrap();
    fs::create_dir_all(&workspace).unwrap();

    let kernel = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "cli-binary-conformance".to_string(),
            created_by: "conformance".to_string(),
            title: "CLI binary non-empty projection".to_string(),
            description: "Seed thread and process state for CLI binary projection".to_string(),
            acceptance_criteria: vec![
                "status reads the thread through hostd".to_string(),
                "process list reads the exited process through hostd".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Seed CLI binary state".to_string(),
            description: "Seed durable thread and process projection state".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::CommandLog],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "conformance".to_string(),
            goal: "seed cli binary status projection".to_string(),
            success_criteria: Vec::new(),
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let environment = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &environment.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            &task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    let capability = kernel
        .grant_capability(
            &agent.agent_id,
            &task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:run_command".to_string()],
            4,
            None,
        )
        .unwrap();
    let command = if cfg!(windows) {
        "Write-Output cli-process-ready"
    } else {
        "echo cli-process-ready"
    };
    let invocation = kernel
        .invoke_tool(
            &agent.agent_id,
            &task.task_id,
            &agent.session_id,
            capability.capability_id,
            4,
            ToolInvokeInput {
                tool_name: "run_command".to_string(),
                input: json!({
                    "command": command,
                    "cwd": workspace.to_string_lossy(),
                }),
                evidence_claim: Some("seeded exited process for CLI projection".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        invocation.status,
        ToolCallStatus::Completed,
        "seed run_command failed: {invocation:#?}"
    );
    let process_id = invocation
        .output
        .as_ref()
        .and_then(|output| output.get("process_id"))
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    wait_for_process_state(&kernel, &process_id, ProcessLifecycleState::Exited);

    let status_output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("status")
        .arg("--thread-id")
        .arg(&agent.thread_id)
        .arg("--state-db")
        .arg(&state_db)
        .output()
        .unwrap();
    assert!(
        status_output.status.success(),
        "agent-os status --thread-id failed with status {}\nstdout:\n{}\nstderr:\n{}",
        status_output.status,
        String::from_utf8_lossy(&status_output.stdout),
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status: Value = serde_json::from_slice(&status_output.stdout).unwrap();
    assert_eq!(status["state_db"], state_db.to_string_lossy().to_string());
    assert_eq!(status["thread"]["client_thread_id"], agent.thread_id);
    assert_eq!(status["thread"]["status"], "Created");
    assert!(status["process_sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|process| process["process_id"] == process_id && process["state"] == "exited"));

    let process_output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("process")
        .arg("list")
        .arg("--state")
        .arg("exited")
        .arg("--state-db")
        .arg(&state_db)
        .output()
        .unwrap();
    assert!(
        process_output.status.success(),
        "agent-os process list --state exited failed with status {}\nstdout:\n{}\nstderr:\n{}",
        process_output.status,
        String::from_utf8_lossy(&process_output.stdout),
        String::from_utf8_lossy(&process_output.stderr)
    );
    let process_list: Value = serde_json::from_slice(&process_output.stdout).unwrap();
    assert_eq!(process_list["action"], "list");
    assert!(process_list["process_sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|process| process["process_id"] == process_id && process["state"] == "exited"));

    drop(kernel);
    remove_dir_all_retry(&root);
}

#[test]
fn cli_run_binary_executes_external_model_through_hostd_and_replays_sqlite_state() {
    let root = isolated_temp_dir("cli-run-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let state_db = root.join("state").join("agent-os.sqlite");
    fs::create_dir_all(state_db.parent().unwrap()).unwrap();
    let model_program = compile_external_run_model(&root);

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("run")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--task")
        .arg("Write result.md from CLI binary conformance")
        .arg("--output")
        .arg("result.md")
        .arg("--bundle-output")
        .arg("bundle/run.json")
        .arg("--state-db")
        .arg(&state_db)
        .arg("--model-command")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "agent-os run failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "completed");
    assert_eq!(value["runtime_job_status"], "completed");
    assert_eq!(value["state_db"], state_db.to_string_lossy().to_string());
    assert_eq!(
        fs::read_to_string(workspace.join("result.md")).unwrap(),
        "cli binary run completed\n"
    );
    let bundle_path = workspace.join("bundle").join("run.json");
    assert_eq!(
        PathBuf::from(value["bundle_path"].as_str().unwrap()),
        bundle_path
    );
    let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], value["task_id"]);

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let task_id = value["task_id"].as_str().unwrap();
    assert!(replayed_state.tasks.contains_key(task_id));
    assert!(replayed_state.final_submissions.contains_key(task_id));
    assert!(!value["artifact_ids"].as_array().unwrap().is_empty());
    assert!(!value["evidence_ids"].as_array().unwrap().is_empty());

    drop(replayed);
    remove_dir_all_retry(&root);
}

#[test]
fn cli_code_binary_applies_exact_edit_runs_verifier_and_replays_sqlite_state() {
    let root = isolated_temp_dir("cli-code-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let workspace = root.join("workspace");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src").join("lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let state_db = root.join("state").join("agent-os.sqlite");
    fs::create_dir_all(state_db.parent().unwrap()).unwrap();
    let model_program = compile_external_code_model(&root);
    let test_program = env::current_exe().unwrap();

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("code")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--task")
        .arg("Change answer from one to two")
        .arg("--file")
        .arg("src/lib.rs")
        .arg("--old")
        .arg("1")
        .arg("--new")
        .arg("2")
        .arg("--test-program")
        .arg(&test_program)
        .arg("--test-arg")
        .arg("--help")
        .arg("--bundle-output")
        .arg("bundle/code.json")
        .arg("--state-db")
        .arg(&state_db)
        .arg("--model-command")
        .arg(&model_program)
        .arg("--model-arg")
        .arg(&test_program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "agent-os code failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "completed");
    assert_eq!(value["runtime_status"], "Completed");
    assert_eq!(value["runtime_job_status"], "completed");
    assert_eq!(value["edit_plan_source"], "exact_prompt");
    assert_eq!(value["planned_file"], "src/lib.rs");
    assert_eq!(value["state_db"], state_db.to_string_lossy().to_string());
    assert_eq!(
        value["model_command"],
        model_program.to_string_lossy().to_string()
    );
    assert_eq!(
        fs::read_to_string(workspace.join("src").join("lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    let changed_path = PathBuf::from(value["changed_path"].as_str().unwrap());
    assert_eq!(changed_path, workspace.join("src").join("lib.rs"));
    let bundle_path = workspace.join("bundle").join("code.json");
    assert_eq!(
        PathBuf::from(value["bundle_path"].as_str().unwrap()),
        bundle_path
    );
    let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], value["task_id"]);
    assert!(!value["artifact_ids"].as_array().unwrap().is_empty());
    assert!(value["evidence_ids"].as_array().unwrap().len() >= 2);
    assert!(value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| {
            evidence["claim"] == "code binary model updated src/lib.rs through apply_patch"
                && evidence["evidence_type"] == "diff_ref"
        }));
    assert!(value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| {
            evidence["claim"] == "code binary validation command ran"
                && evidence["evidence_type"] == "command_log"
                && evidence["metadata"]["output"]["exit_code"] == 0
        }));

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let task_id = value["task_id"].as_str().unwrap();
    let thread_id = value["thread_id"].as_str().unwrap();
    assert!(replayed_state.tasks.contains_key(task_id));
    assert!(replayed_state.final_submissions.contains_key(task_id));
    assert_eq!(
        replayed_state.threads.get(thread_id).unwrap().status,
        ThreadStatus::Completed
    );

    drop(replayed);
    remove_dir_all_retry(&root);
}

#[test]
fn cli_chat_binary_runs_task_through_configured_provider_and_replays_sqlite_state() {
    let root = isolated_temp_dir("cli-chat-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let state_db = root.join("state").join("agent-os.sqlite");
    fs::create_dir_all(state_db.parent().unwrap()).unwrap();
    let agent_home = root.join("agent-home");
    fs::create_dir_all(agent_home.join("config")).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    fs::write(
        agent_home.join("config").join("config.json"),
        serde_json::to_vec_pretty(&json!({
            "model": "local/chat-model",
            "provider": {
                "local": {
                    "api_key": "test-key",
                    "endpoint": "openai_chat_completions",
                    "options": {
                        "base_url": endpoint,
                        "timeout_ms": 5000
                    },
                    "models": {
                        "chat-model": {
                            "name": "wire-chat-model",
                            "options": {
                                "reasoningEffort": "medium",
                                "max_tokens": 999999
                            },
                            "limit": {"context": 65536, "output": 1024},
                            "capabilities": {
                                "streaming": true,
                                "tool_calling": true,
                                "reasoning": false,
                                "temperature": true,
                                "image_input": false,
                                "structured_output": false
                            }
                        }
                    }
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let (requests_tx, requests_rx) = mpsc::channel();
    let server = thread::spawn(move || serve_chat_provider_endpoint(listener, requests_tx));

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("chat")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--task")
        .arg("Create chat_result.txt from chat binary conformance")
        .arg("--bundle-output")
        .arg("bundle/chat.json")
        .arg("--state-db")
        .arg(&state_db)
        .arg("--max-steps")
        .arg("6")
        .arg("--runtime-timeout-seconds")
        .arg("30")
        .arg("--temperature")
        .arg("0.2")
        .env("AGENT_OS_HOME", &agent_home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "agent-os chat failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Agent-OS v0.3"));
    assert!(stdout.contains("Provider:  local"));
    assert!(stdout.contains("Model:     local/chat-model"));
    assert!(stdout.contains("Session: 1 task(s)"));
    assert_eq!(
        fs::read_to_string(workspace.join("chat_result.txt")).unwrap(),
        "CHAT_BINARY_OK\n"
    );
    let bundle_path = workspace.join("bundle").join("chat.json");
    let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
    let task_id = bundle["root_task_id"].as_str().unwrap().to_string();
    assert_eq!(
        bundle["replay_summary"]["final_submission_count"].as_u64(),
        Some(1)
    );
    let captured_requests = requests_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    server.join().unwrap();
    assert_eq!(captured_requests.len(), 2);
    assert_eq!(captured_requests[0]["__http"]["path"], "/chat/completions");
    assert_eq!(
        captured_requests[0]["__http"]["headers"]["authorization"],
        "Bearer test-key"
    );
    assert_eq!(captured_requests[0]["model"], "wire-chat-model");
    assert_eq!(captured_requests[0]["reasoningEffort"], "medium");
    assert_eq!(captured_requests[0]["max_tokens"], 1024);
    assert_eq!(captured_requests[0]["temperature"], 0.2);
    assert!(captured_requests[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["function"]["name"] == "apply_patch"));

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state.final_submissions.contains_key(&task_id));
    assert!(replayed_state
        .threads
        .values()
        .any(|thread| thread.status == ThreadStatus::Completed));

    drop(replayed);
    remove_dir_all_retry(&root);
}

#[test]
fn cli_resume_binary_recovers_running_thread_and_completes_runtime_job_through_hostd() {
    let root = isolated_temp_dir("cli-resume-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let state_db = root.join("state").join("agent-os.sqlite");
    fs::create_dir_all(state_db.parent().unwrap()).unwrap();
    let model_program = compile_external_run_model(&root);

    let kernel = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let goal = kernel
        .register_goal(RegisterGoalInput {
            namespace: "cli-binary-conformance".to_string(),
            created_by: "conformance".to_string(),
            title: "CLI binary resume".to_string(),
            description: "Seed a resumable runtime thread for the CLI resume binary".to_string(),
            acceptance_criteria: vec![
                "resume recovers the running thread through hostd".to_string(),
                "resume completes a runtime job through the configured model command".to_string(),
            ],
            constraints: Vec::new(),
            risk_level: 4,
            deadline: None,
        })
        .unwrap();
    let task = kernel
        .spawn_task(SpawnTaskInput {
            goal_id: goal.goal_id.clone(),
            parent_task_id: None,
            title: "Resume CLI binary state".to_string(),
            description: "Seed durable state for CLI resume conformance".to_string(),
            depends_on: Vec::new(),
            required_artifact_types: Vec::new(),
            required_evidence_types: vec![EvidenceType::DiffRef],
            priority: 10,
            risk_level: 4,
        })
        .unwrap();
    let agent = kernel
        .spawn_agent(SpawnAgentInput {
            task_id: task.task_id.clone(),
            role_profile_id: "role_producer".to_string(),
            owner: "conformance".to_string(),
            goal: "resume and write result.md".to_string(),
            success_criteria: vec!["result.md exists".to_string()],
            failure_criteria: Vec::new(),
            parent_thread_id: None,
            workspace_roots: vec![workspace.to_string_lossy().to_string()],
        })
        .unwrap();
    let environment = kernel
        .create_environment(
            BackendType::IsolatedWorktree,
            workspace.to_string_lossy(),
            "sbox_workspace_write",
            ReusePolicy::TaskScoped,
        )
        .unwrap();
    kernel
        .attach_environment(
            &environment.environment_id,
            &agent.agent_id,
            &agent.thread_id,
            &task.task_id,
            AttachMode::WorkspaceWrite,
        )
        .unwrap();
    kernel
        .grant_capability(
            &agent.agent_id,
            &task.task_id,
            vec!["tool.invoke".to_string()],
            vec!["tool:apply_patch".to_string()],
            4,
            None,
        )
        .unwrap();
    kernel.start_turn(&agent.thread_id).unwrap();
    drop(kernel);

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("resume")
        .arg("--thread-id")
        .arg(&agent.thread_id)
        .arg("--workspace")
        .arg(&workspace)
        .arg("--bundle-output")
        .arg("bundle/resume.json")
        .arg("--state-db")
        .arg(&state_db)
        .arg("--model-command")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "agent-os resume failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "completed");
    assert_eq!(value["thread_id"], agent.thread_id);
    assert_eq!(value["task_id"], task.task_id);
    assert_eq!(value["previous_thread_status"], "Running");
    assert_eq!(value["runtime_status"], "Completed");
    assert_eq!(value["runtime_job_status"], "completed");
    assert_eq!(
        fs::read_to_string(workspace.join("result.md")).unwrap(),
        "cli binary run completed\n"
    );
    let bundle_path = workspace.join("bundle").join("resume.json");
    assert_eq!(
        PathBuf::from(value["bundle_path"].as_str().unwrap()),
        bundle_path
    );
    let bundle: Value = serde_json::from_slice(&fs::read(bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], task.task_id);
    assert!(value["turns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|turn| turn["status"] == "Completed"));
    assert!(value["runtime_jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|job| job["status"] == "completed"));

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state.final_submissions.contains_key(&task.task_id));
    assert_eq!(
        replayed_state.threads.get(&agent.thread_id).unwrap().status,
        ThreadStatus::Completed
    );

    drop(replayed);
    remove_dir_all_retry(&root);
}

fn build_cli_and_hostd_binaries(target_dir: &Path) {
    static BUILD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = BUILD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let shared_target_dir = workspace_root()
        .join("target")
        .join("agent-os-conformance-cli-binaries");
    if !binary_path(&shared_target_dir, "agent-os").exists()
        || !binary_path(&shared_target_dir, "agent-os-hostd").exists()
    {
        build_shared_cli_and_hostd_binaries(&shared_target_dir);
    }
    let binary_dir = target_dir.join("debug");
    fs::create_dir_all(&binary_dir).unwrap();
    for stem in ["agent-os", "agent-os-hostd"] {
        fs::copy(
            binary_path(&shared_target_dir, stem),
            binary_path(target_dir, stem),
        )
        .unwrap();
    }
}

fn build_shared_cli_and_hostd_binaries(target_dir: &Path) {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("build")
        .arg("-p")
        .arg("agent-os-cli")
        .arg("--bin")
        .arg("agent-os")
        .arg("-p")
        .arg("agent-os-host")
        .arg("--bin")
        .arg("agent-os-hostd")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(workspace_root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo build for CLI conformance failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn binary_path(target_dir: &Path, stem: &str) -> PathBuf {
    target_dir.join("debug").join(if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    })
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

fn isolated_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "agent-os-conformance-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn compile_external_run_model(root: &Path) -> PathBuf {
    let source_path = root.join("external_run_model.rs");
    let model_program = root.join(format!(
        "external_run_model{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
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
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"*** Begin Patch\\n*** Add File: result.md\\n+cli binary run completed\\n*** End Patch\\n\"}},\"risk_level\":4,\"evidence_claim\":\"cli binary run wrote result.md through apply_patch\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"CLI binary run completed.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"result.md was written\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[\"agent-os run binary conformance\"],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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
    let output = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc external model failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    model_program
}

fn compile_external_code_model(root: &Path) -> PathBuf {
    let source_path = root.join("external_code_model.rs");
    let model_program = root.join(format!(
        "external_code_model{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
        &source_path,
        r##"
use std::env;
use std::io::{self, Read};

fn main() {
    let test_program = env::args().nth(1).expect("test program arg");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    match step_index(&input) {
        0 => {
            let workspace_root = json_string(&input, "workspace_root");
            let patch = "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn answer() -> i32 { 1 }\n+pub fn answer() -> i32 { 2 }\n*** End Patch\n";
            print!(
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"apply_patch\",\"input\":{{\"workspace_root\":\"{}\",\"patch\":\"{}\"}},\"risk_level\":4,\"evidence_claim\":\"code binary model updated src/lib.rs through apply_patch\"}},{{\"type\":\"tool_call\",\"tool_name\":\"run_command\",\"input\":{{\"mode\":\"exec\",\"command\":\"{}\",\"args\":[\"--help\"],\"cwd\":\"{}\"}},\"risk_level\":4,\"evidence_claim\":\"code binary validation command ran\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root,
                json_escape(patch),
                json_escape(&test_program),
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"Code binary conformance completed.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"src/lib.rs was edited and verifier ran\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[\"agent-os code binary conformance\"],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
"##,
    )
    .unwrap();
    let output = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc external code model failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    model_program
}

fn serve_chat_provider_endpoint(listener: TcpListener, requests_tx: mpsc::Sender<Vec<Value>>) {
    let mut requests = Vec::new();
    for step_index in 0..2 {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let request = read_http_json(&mut stream);
        let response = if step_index == 0 {
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_chat_patch",
                            "type": "function",
                            "function": {
                                "name": "apply_patch",
                                "arguments": "{\"patch\":\"*** Begin Patch\\n*** Add File: chat_result.txt\\n+CHAT_BINARY_OK\\n*** End Patch\\n\"}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 100, "completion_tokens": 8}
            })
        } else {
            let evidence_refs = evidence_refs_from_openai_request(&request);
            assert!(
                !evidence_refs.is_empty(),
                "second chat provider request must include tool evidence ids"
            );
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_chat_final",
                            "type": "function",
                            "function": {
                                "name": "submit_final",
                                "arguments": json!({
                                    "summary": "Chat binary conformance completed.",
                                    "changed_artifacts": [],
                                    "evidence_map": [{
                                        "claim": "chat_result.txt was created through apply_patch",
                                        "evidence_refs": evidence_refs
                                    }],
                                    "unverified_claims": [],
                                    "known_risks": [],
                                    "tests_run": ["agent-os chat binary conformance"],
                                    "tests_not_run": [],
                                    "approvals": []
                                }).to_string()
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 120, "completion_tokens": 12}
            })
        };
        write_http_json(&mut stream, &response);
        requests.push(request);
    }
    requests_tx.send(requests).unwrap();
}

fn read_http_json(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let (header_end, content_length, path, headers_value) = loop {
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "connection closed before headers");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = find_header_end(&bytes) {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let request_path = headers
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .expect("request path")
                .to_string();
            let headers_value = headers
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    Some((name.trim().to_ascii_lowercase(), json!(value.trim())))
                })
                .collect::<serde_json::Map<String, Value>>();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("content-length header");
            break (
                header_end,
                content_length,
                request_path,
                Value::Object(headers_value),
            );
        }
    };

    let body_start = header_end + 4;
    while bytes.len() < body_start + content_length {
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "connection closed before body");
        bytes.extend_from_slice(&buffer[..read]);
    }
    let mut body: Value =
        serde_json::from_slice(&bytes[body_start..body_start + content_length]).unwrap();
    body.as_object_mut()
        .expect("provider request body must be an object")
        .insert(
            "__http".to_string(),
            json!({
                "path": path,
                "headers": headers_value,
            }),
        );
    body
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

fn evidence_refs_from_openai_request(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|message| message["role"] == "tool")
        .filter_map(|message| message["content"].as_str())
        .filter_map(|content| serde_json::from_str::<Value>(content).ok())
        .flat_map(|content| {
            content["evidence_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn wait_for_process_state(
    kernel: &Kernel,
    process_id: &str,
    expected: ProcessLifecycleState,
) -> ProcessSession {
    let started = std::time::Instant::now();
    loop {
        let session = kernel
            .state_snapshot()
            .unwrap()
            .process_sessions
            .get(process_id)
            .unwrap()
            .clone();
        if session.state == expected {
            return session;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "process {process_id} remained in {:?}, expected {:?}",
            session.state,
            expected
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn remove_dir_all_retry(path: &Path) {
    for attempt in 0..10 {
        match fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(error) if attempt < 9 => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if !path.exists() {
                    return;
                }
                if attempt == 8 {
                    eprintln!("retrying cleanup of {} after {error}", path.display());
                }
            }
            Err(error) => panic!("remove_dir_all {}: {error}", path.display()),
        }
    }
}
