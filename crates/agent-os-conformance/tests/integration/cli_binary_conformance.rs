use crate::common::*;
use agent_os_store_sqlite::SqliteStore;
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
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

fn build_cli_and_hostd_binaries(target_dir: &Path) {
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
