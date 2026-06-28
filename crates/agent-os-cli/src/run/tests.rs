use super::*;
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::new_id;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn cli_run_completes_workspace_task_and_writes_output() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-run-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let options = RunOptions {
        workspace: workspace.clone(),
        task: "Write a deterministic task report".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: Some(PathBuf::from("bundle.json")),
        state_db: None,
        model_command: None,
        model_args: Vec::new(),
    };
    let output = run_e2e_task(&options).unwrap();
    assert_eq!(output["status"], json!("completed"));
    assert!(workspace.join("result.md").exists());
    assert!(workspace.join("bundle.json").exists());
    let bundle: Value =
        serde_json::from_str(&fs::read_to_string(workspace.join("bundle.json")).unwrap()).unwrap();
    assert_eq!(bundle["bundle_kind"], json!("task"));
    assert_eq!(bundle["root_task_id"], output["task_id"]);
    assert_eq!(output["replay"]["final_submissions"], json!(1));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn cli_run_rejects_output_path_escape() {
    let options = RunOptions {
        workspace: PathBuf::from("."),
        task: "bad output".to_string(),
        output: PathBuf::from("../escape.md"),
        bundle_output: None,
        state_db: None,
        model_command: None,
        model_args: Vec::new(),
    };
    let err = run_e2e_task(&options).unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
}

#[test]
fn cli_run_persists_events_to_state_db_for_restart_replay() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-state-db-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    let state_db = workspace.join("agent-os.sqlite");
    let options = RunOptions {
        workspace: workspace.clone(),
        task: "Write a durable task report".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: None,
        state_db: Some(state_db.clone()),
        model_command: None,
        model_args: Vec::new(),
    };
    let output = run_e2e_task(&options).unwrap();
    assert_eq!(output["status"], json!("completed"));
    assert!(state_db.exists());

    let replayed = Kernel::with_replayed_store(SqliteStore::open(&state_db).unwrap()).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    let task_id = output["task_id"].as_str().unwrap();
    assert!(replayed_state.tasks.contains_key(task_id));
    assert!(replayed_state.final_submissions.contains_key(task_id));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn cli_run_can_use_external_model_process() {
    let workspace = env::temp_dir().join(format!(
        "agent-os-cli-external-run-{}-{}",
        std::process::id(),
        new_id("case_")
    ));
    fs::create_dir_all(&workspace).unwrap();
    let source_path = workspace.join("external_run_model.rs");
    let model_program = workspace.join(format!(
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
                "{{\"actions\":[{{\"type\":\"tool_call\",\"tool_name\":\"write_file\",\"input\":{{\"workspace_root\":\"{}\",\"path\":\"result.md\",\"content\":\"external model completed\\n\"}},\"risk_level\":4,\"evidence_claim\":\"external model wrote result file\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
                workspace_root
            );
        }
        _ => {
            let evidence_id = first_evidence_id(&input);
            print!(
                "{{\"actions\":[{{\"type\":\"final\",\"submission\":{{\"summary\":\"External model completed the task.\",\"changed_artifacts\":[],\"evidence_map\":[{{\"claim\":\"result file was written\",\"evidence_refs\":[\"{}\"]}}],\"unverified_claims\":[],\"known_risks\":[],\"tests_run\":[],\"tests_not_run\":[],\"approvals\":[]}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1,\"cost\":0.0}}}}",
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
    let rustc_output = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&model_program)
        .output()
        .unwrap();
    assert!(
        rustc_output.status.success(),
        "rustc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    let options = RunOptions {
        workspace: workspace.clone(),
        task: "Write result.md with an external model".to_string(),
        output: PathBuf::from("result.md"),
        bundle_output: None,
        state_db: None,
        model_command: Some(model_program),
        model_args: Vec::new(),
    };
    let output = run_e2e_task(&options).unwrap();
    assert_eq!(output["status"], json!("completed"));
    assert_eq!(
        fs::read_to_string(workspace.join("result.md")).unwrap(),
        "external model completed\n"
    );
    assert_eq!(output["replay"]["final_submissions"], json!(1));
    assert_eq!(output["artifact_ids"].as_array().unwrap().len(), 1);
    let _ = fs::remove_dir_all(workspace);
}
