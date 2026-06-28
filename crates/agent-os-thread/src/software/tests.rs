use super::*;
use agent_os_kernel::Kernel;
use agent_os_sys::*;
use std::path::PathBuf;
use std::{env, fs};

#[test]
fn software_pipeline_runs_all_roles_and_submits_supervisor_final() {
    let workspace = temp_workspace("agent-os-software-pipeline");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new());
    let report = pipeline
        .run_code_task(SoftwareCodeTask::exact_edit(
            &workspace,
            "Change answer from one to two",
            "src/lib.rs",
            "1",
            "2",
            env::current_exe().unwrap(),
            vec!["--help".to_string()],
        ))
        .unwrap();

    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.test_exit_code, 0);
    assert_eq!(report.verification_verdict, VerificationVerdict::Pass);
    for role in ["SupervisorAgent", "WorkerAgent", "ReviewerAgent"] {
        assert!(report.role_thread_ids.contains_key(role), "missing {role}");
    }
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    assert_eq!(report.replay.final_submissions, 6);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn review_finding_triggers_revision_before_supervisor_final() {
    let workspace = temp_workspace("agent-os-software-revision");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new());
    let mut spec = SoftwareCodeTask::exact_edit(
        &workspace,
        "Change answer through review",
        "src/lib.rs",
        "1",
        "2",
        env::current_exe().unwrap(),
        vec!["--help".to_string()],
    );
    spec.review_revision = Some(ReviewRevision {
        finding_title: "Use final accepted value".to_string(),
        finding_body: "The reviewer requires answer 42 instead of 2.".to_string(),
        old: "{ 2 }".to_string(),
        new: "{ 42 }".to_string(),
    });
    let report = pipeline.run_code_task(spec).unwrap();

    assert_eq!(report.review_finding_count, 1);
    assert_eq!(
        report.review_verdicts,
        vec![ReviewVerdict::NeedsRevision, ReviewVerdict::Accept]
    );
    assert_eq!(report.artifact_ids.len(), 2);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 42 }\n"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn failed_test_blocks_supervisor_final() {
    let workspace = temp_workspace("agent-os-software-failed-test");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new());
    let err = pipeline
        .run_code_task(SoftwareCodeTask::exact_edit(
            &workspace,
            "Change answer but fail test",
            "src/lib.rs",
            "1",
            "2",
            env::current_exe().unwrap(),
            vec!["--definitely-not-a-test-binary-flag".to_string()],
        ))
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
    let state = pipeline.kernel().state_snapshot().unwrap();
    assert_eq!(state.final_submissions.len(), 2);
    assert!(
        state
            .threads
            .values()
            .all(|thread| thread.role != "SupervisorAgent"
                || thread.status != ThreadStatus::Completed)
    );
    let _ = fs::remove_dir_all(workspace);
}

fn temp_workspace(prefix: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}
