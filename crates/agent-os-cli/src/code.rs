use crate::args::CodeOptions;
use crate::support::{
    ensure_safe_relative_workspace_path, io_result, open_kernel, write_task_bundle_if_requested,
};
use agent_os_sys::*;
use agent_os_thread::{SoftwareCodeTask, SoftwareEngineeringPipeline};
use serde_json::{json, Value};
use std::fs;

pub(crate) fn run_code_task(options: &CodeOptions) -> AgentOsResult<Value> {
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    let spec = match (&options.file, &options.old, &options.new) {
        (Some(file), Some(old), Some(new)) => {
            ensure_safe_relative_workspace_path(file, "--file")?;
            SoftwareCodeTask::exact_edit(
                &options.workspace,
                &options.task,
                file,
                old,
                new,
                &options.test_program,
                options.test_args.clone(),
            )
        }
        (file, None, None) => {
            if let Some(file) = file {
                ensure_safe_relative_workspace_path(file, "--file")?;
            }
            SoftwareCodeTask::plan_from_task(
                &options.workspace,
                &options.task,
                file.clone(),
                &options.test_program,
                options.test_args.clone(),
            )?
        }
        _ => {
            return Err(AgentOsError::Validation(
                "--old and --new must be provided together".to_string(),
            ));
        }
    };
    let target_path = options.workspace.join(&spec.file);
    if !target_path.exists() {
        return Err(AgentOsError::NotFound(format!(
            "target file {}",
            target_path.to_string_lossy()
        )));
    }

    let pipeline = SoftwareEngineeringPipeline::new(open_kernel(&options.state_db)?);
    let report = pipeline.run_code_task(spec)?;
    let bundle_path = write_task_bundle_if_requested(
        &pipeline.kernel(),
        &report.supervisor_final_task_id,
        &options.workspace,
        &options.bundle_output,
    )?;
    Ok(json!({
        "status": "completed",
        "goal_id": report.goal_id,
        "changed_path": target_path.to_string_lossy(),
        "state_db": options.state_db.as_ref().map(|path| path.to_string_lossy().to_string()),
        "bundle_path": bundle_path,
        "planned_file": report.planned_file,
        "edit_plan_source": report.edit_plan_source,
        "latest_artifact_id": report.latest_artifact_id,
        "artifact_ids": report.artifact_ids,
        "evidence_ids": report.evidence_ids,
        "test_exit_code": report.test_exit_code,
        "role_thread_ids": report.role_thread_ids,
        "review_verdicts": report.review_verdicts,
        "review_finding_count": report.review_finding_count,
        "verification_verdict": report.verification_verdict,
        "supervisor_final_task_id": report.supervisor_final_task_id,
        "events": report.events,
        "replay": report.replay
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_sys::new_id;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn cli_code_applies_exact_edit_and_runs_test_command() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-code-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src/lib.rs"),
            "pub fn answer() -> i32 { 1 }\n",
        )
        .unwrap();
        let options = CodeOptions {
            workspace: workspace.clone(),
            task: "Change answer from one to two".to_string(),
            file: Some(PathBuf::from("src/lib.rs")),
            old: Some("1".to_string()),
            new: Some("2".to_string()),
            test_program: env::current_exe().unwrap(),
            test_args: vec!["--help".to_string()],
            bundle_output: None,
            state_db: None,
        };
        let output = run_code_task(&options).unwrap();
        assert_eq!(output["status"], json!("completed"));
        assert_eq!(
            fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> i32 { 2 }\n"
        );
        assert_eq!(output["replay"]["final_submissions"], json!(6));
        for role in ["SupervisorAgent", "WorkerAgent", "ReviewerAgent"] {
            assert!(
                output["role_thread_ids"].get(role).is_some(),
                "missing {role}"
            );
        }
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn cli_code_can_infer_simple_edit_from_task() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-cli-code-plan-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src/lib.rs"),
            "pub fn answer() -> i32 { 1 }\n",
        )
        .unwrap();
        let options = CodeOptions {
            workspace: workspace.clone(),
            task: "Change answer from one to two".to_string(),
            file: None,
            old: None,
            new: None,
            test_program: env::current_exe().unwrap(),
            test_args: vec!["--help".to_string()],
            bundle_output: None,
            state_db: None,
        };
        let output = run_code_task(&options).unwrap();
        assert_eq!(output["status"], json!("completed"));
        assert_eq!(output["edit_plan_source"], json!("inferred"));
        assert_eq!(
            fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
            "pub fn answer() -> i32 { 2 }\n"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}
