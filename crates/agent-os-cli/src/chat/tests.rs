use super::*;
use std::env;
use std::fs;

#[test]
fn chat_session_reports_summary() {
    let workspace = env::temp_dir().join(format!(
        "aos-chat-test-{}-{}",
        std::process::id(),
        new_id("c_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let kernel = Kernel::new();
    let mut session = ChatSession::new(
        kernel,
        workspace.clone(),
        "test-provider".to_string(),
        "test-model".to_string(),
    );
    session.task_count = 3;
    session.total_events = 42;
    let summary = session.summary();
    assert_eq!(summary["tasks"], 3);
    assert_eq!(summary["total_events"], 42);
    assert_eq!(summary["provider"], "test-provider");
    assert_eq!(summary["model"], "test-model");
    let _ = fs::remove_dir_all(workspace);
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
    let record = agent_os_thread::ToolExecutionRecord {
        call_id: "c1".to_string(),
        tool_name: "read_file".to_string(),
        status: ToolCallStatus::Completed,
        input: None,
        output: Some(json!({
            "path": "src/main.rs",
            "content": "fn main() {}",
            "bytes_read": 13,
        })),
        evidence_ids: vec![],
        evidence_claim: None,
    };
    let summary = format_tool_summary(&record);
    assert!(summary.contains("read"));
    assert!(summary.contains("src/main.rs"));
}

#[test]
fn format_tool_summary_shows_replace_count() {
    let record = agent_os_thread::ToolExecutionRecord {
        call_id: "c2".to_string(),
        tool_name: "replace_text".to_string(),
        status: ToolCallStatus::Completed,
        input: None,
        output: Some(json!({
            "changed_path": "lib.rs",
            "replacements": 3,
            "before": "old",
            "after": "new",
        })),
        evidence_ids: vec![],
        evidence_claim: None,
    };
    let summary = format_tool_summary(&record);
    assert!(summary.contains("edit"));
    assert!(summary.contains("3 replacement"));
}

#[test]
fn format_tool_summary_shows_process_exit_code() {
    let record = agent_os_thread::ToolExecutionRecord {
        call_id: "c3".to_string(),
        tool_name: "run_command".to_string(),
        status: ToolCallStatus::Completed,
        input: None,
        output: Some(json!({
            "exit_code": 0,
            "stdout": "all good",
            "stderr": "",
            "input": {"program": "cargo", "args": ["test"]},
        })),
        evidence_ids: vec![],
        evidence_claim: None,
    };
    let summary = format_tool_summary(&record);
    assert!(summary.contains("run"));
    assert!(summary.contains("cargo"));
    assert!(summary.contains("exit 0"));
}

#[test]
fn format_tool_summary_shows_delete_path() {
    let record = agent_os_thread::ToolExecutionRecord {
        call_id: "c4".to_string(),
        tool_name: "delete_file".to_string(),
        status: ToolCallStatus::Completed,
        input: None,
        output: Some(json!({
            "deleted_path": "old.txt",
            "deleted_bytes": 12,
        })),
        evidence_ids: vec![],
        evidence_claim: None,
    };
    let summary = format_tool_summary(&record);
    assert!(summary.contains("delete"));
    assert!(summary.contains("old.txt"));
}
