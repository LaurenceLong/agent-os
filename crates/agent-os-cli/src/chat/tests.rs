use super::*;
use std::env;
use std::fs;

#[test]
fn chat_session_process_task_uses_app_client_projection_contract() {
    let workspace = env::temp_dir().join(format!(
        "aos-chat-app-client-test-{}-{}",
        std::process::id(),
        new_id("c_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let mut session = ChatSession::new_for_app_client(
        Box::new(FakeChatClient::default()),
        workspace.clone(),
        "test-provider".to_string(),
        "general-primary".to_string(),
    )
    .unwrap();
    let options = ChatOptions {
        workspace: workspace.clone(),
        task: None,
        task_file: None,
        model: None,
        max_steps: 4,
        runtime_timeout_seconds: 120,
        max_tokens: None,
        temperature: None,
        state_db: None,
        bundle_output: None,
    };

    session
        .process_task("Complete a chat task", &options)
        .unwrap();

    let summary = session.summary();
    assert_eq!(summary["tasks"], json!(1));
    assert_eq!(summary["total_events"], json!(1));
    assert_eq!(summary["last_thread_id"], json!("thread_1"));
    assert_eq!(summary["last_task_id"], json!("task_1"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn chat_session_exports_bundle_when_requested() {
    let workspace = env::temp_dir().join(format!(
        "aos-chat-bundle-test-{}-{}",
        std::process::id(),
        new_id("c_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let mut session = ChatSession::new_for_app_client(
        Box::new(FakeChatClient::default()),
        workspace.clone(),
        "test-provider".to_string(),
        "general-primary".to_string(),
    )
    .unwrap();
    let options = ChatOptions {
        workspace: workspace.clone(),
        task: None,
        task_file: None,
        model: None,
        max_steps: 4,
        runtime_timeout_seconds: 120,
        max_tokens: None,
        temperature: None,
        state_db: None,
        bundle_output: Some(std::path::PathBuf::from("bundle/chat.json")),
    };

    session
        .process_task("Complete a chat task", &options)
        .unwrap();

    let bundle_path = workspace.join("bundle/chat.json");
    let summary = session.summary();
    assert_eq!(
        summary["last_bundle_path"],
        json!(bundle_path.to_string_lossy())
    );
    let bundle: Value = serde_json::from_slice(&fs::read(&bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["root_task_id"], "task_1");
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn chat_session_reports_summary() {
    let workspace = env::temp_dir().join(format!(
        "aos-chat-test-{}-{}",
        std::process::id(),
        new_id("c_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let mut session = ChatSession::new_for_app_client(
        Box::new(FakeChatClient::default()),
        workspace.clone(),
        "test-provider".to_string(),
        "general-primary".to_string(),
    )
    .unwrap();
    session.task_count = 3;
    session.total_events = 42;
    let summary = session.summary();
    assert_eq!(summary["tasks"], 3);
    assert_eq!(summary["total_events"], 42);
    assert_eq!(summary["provider"], "test-provider");
    assert_eq!(summary["model"], "general-primary");
    let _ = fs::remove_dir_all(workspace);
}

#[derive(Default)]
struct FakeChatClient {
    requests: Vec<&'static str>,
}

impl ChatAppClient for FakeChatClient {
    fn request(&mut self, request: AppRequest) -> AgentOsResult<Value> {
        match request {
            AppRequest::Initialize => {
                self.requests.push("initialize");
                Ok(json!({"initialized": true}))
            }
            AppRequest::ThreadStart { goal, workspace } => {
                self.requests.push("thread/start");
                assert_eq!(goal, "Complete a chat task");
                assert!(workspace.is_some());
                Ok(json!({
                    "thread": {
                        "client_thread_id": "thread_1",
                        "task_id": "task_1",
                        "goal_id": "goal_1",
                        "status": "Ready"
                    }
                }))
            }
            AppRequest::TurnStart {
                client_thread_id,
                input,
            } => {
                self.requests.push("turn/start");
                assert_eq!(client_thread_id, "thread_1");
                assert_eq!(input, "Complete a chat task");
                Ok(json!({
                    "runtime_job": {
                        "runtime_job_id": "rtjob_1",
                        "status": "queued"
                    }
                }))
            }
            AppRequest::ThreadRead { client_thread_id } => {
                self.requests.push("thread/read");
                assert_eq!(client_thread_id, "thread_1");
                Ok(json!({
                    "thread": {
                        "client_thread_id": "thread_1",
                        "task_id": "task_1",
                        "status": "Completed"
                    },
                    "turns": [],
                    "timeline": [
                        {
                            "item_type": "ToolUpdated",
                            "payload": {
                                "tool_name": "run_command",
                                "status": "Completed",
                                "output": {
                                    "exit_code": 0,
                                    "input": {"command": "agent-os"}
                                }
                            }
                        }
                    ],
                    "runtime_jobs": [
                        {
                            "runtime_job_id": "rtjob_1",
                            "status": "completed"
                        }
                    ],
                    "artifacts": [],
                    "evidence": [],
                    "resources": [],
                    "automation_runs": []
                }))
            }
            AppRequest::TaskBundleExport { client_thread_id } => {
                self.requests.push("task/bundle/export");
                assert_eq!(client_thread_id, "thread_1");
                Ok(json!({
                    "bundle": {
                        "abi_version": "0.3.0",
                        "bundle_kind": "task",
                        "exported_at": "2026-06-30T00:00:00Z",
                        "root_task_id": "task_1",
                        "goal_id": "goal_1",
                        "task_ids": ["task_1"],
                        "profile_snapshot": {},
                        "projection_snapshot": {},
                        "events": [],
                        "replay_summary": {
                            "event_count": 1,
                            "task_count": 1,
                            "thread_count": 1,
                            "artifact_count": 0,
                            "evidence_count": 0,
                            "final_submission_count": 0
                        }
                    }
                }))
            }
            other => panic!("unexpected request {other:?}"),
        }
    }
}

#[test]
fn chat_options_with_initial_task_run_in_batch_mode() {
    let mut options =
        ChatOptions::parse(&["--task-file".to_string(), "task.md".to_string()]).unwrap();
    assert!(exits_after_initial_task(&options));

    options.task_file = None;
    options.task = Some("inline task".to_string());
    assert!(exits_after_initial_task(&options));

    options.task = None;
    assert!(!exits_after_initial_task(&options));
}

#[test]
fn format_tool_summary_shows_read_file_path() {
    let record = json!({
        "call_id": "c1",
        "tool_name": "read_file",
        "status": "Completed",
        "input": null,
        "output": {
            "path": "src/main.rs",
            "content": "fn main() {}",
            "bytes_read": 13
        },
        "evidence_ids": [],
        "evidence_claim": null
    });
    let summary = format_tool_summary_value(&record);
    assert!(summary.contains("read"));
    assert!(summary.contains("src/main.rs"));
}

#[test]
fn format_tool_summary_shows_apply_patch_operation() {
    let record = json!({
        "call_id": "c2",
        "tool_name": "apply_patch",
        "status": "Completed",
        "input": null,
        "output": {
            "operation": "update",
            "path": "lib.rs",
            "changed_path": "lib.rs"
        },
        "evidence_ids": [],
        "evidence_claim": null
    });
    let summary = format_tool_summary_value(&record);
    assert!(summary.contains("patch"));
    assert!(summary.contains("update"));
    assert!(summary.contains("lib.rs"));
}

#[test]
fn format_tool_summary_shows_process_exit_code() {
    let record = json!({
        "call_id": "c3",
        "tool_name": "run_command",
        "status": "Completed",
        "input": null,
        "output": {
            "exit_code": 0,
            "stdout": "all good",
            "stderr": "",
            "input": {"command": "cargo test"}
        },
        "evidence_ids": [],
        "evidence_claim": null
    });
    let summary = format_tool_summary_value(&record);
    assert!(summary.contains("run"));
    assert!(summary.contains("cargo"));
    assert!(summary.contains("exit 0"));
}

#[test]
fn format_tool_summary_shows_apply_patch_delete_path() {
    let record = json!({
        "call_id": "c4",
        "tool_name": "apply_patch",
        "status": "Completed",
        "input": null,
        "output": {
            "operation": "delete",
            "path": "old.txt",
            "deleted_path": "old.txt",
            "deleted_bytes": 12
        },
        "evidence_ids": [],
        "evidence_claim": null
    });
    let summary = format_tool_summary_value(&record);
    assert!(summary.contains("patch"));
    assert!(summary.contains("delete"));
    assert!(summary.contains("old.txt"));
}
