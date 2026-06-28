use crate::common::*;
use agent_os_sys::{PackageManifest, PackageType};
use agent_os_thread::{SoftwareCodeTask, SoftwareEditPlanSource, SoftwareEngineeringPipeline};
use std::{env, fs, path::PathBuf};

#[test]
fn software_engineering_distro_package_has_manifest_and_policy_packs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("distros/software-engineering");
    let manifest: PackageManifest =
        serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.package_name, "software-engineering");
    assert_eq!(manifest.package_type, PackageType::Distro);
    assert!(root.join("prompts/supervisor.md").exists());
    assert!(root.join("prompts/worker.md").exists());
    assert!(root.join("prompts/reviewer.md").exists());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(root.join("policy/final-answer.json")).unwrap()
        )
        .unwrap()["policy_name"],
        "software-engineering-final-answer"
    );
}

#[test]
fn software_distribution_runs_through_required_roles() {
    let workspace = temp_workspace("agent-os-conformance-software");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new()).unwrap();
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
    assert_eq!(report.replay.final_submissions, 6);
    assert!(report.replay.reviews >= 1);
    assert!(report.replay.verifications >= 1);
    for role in ["SupervisorAgent", "WorkerAgent", "ReviewerAgent"] {
        assert!(report.role_thread_ids.contains_key(role), "missing {role}");
    }
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn failed_tests_block_supervisor_final_acceptance() {
    let workspace = temp_workspace("agent-os-conformance-failed-test");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new()).unwrap();
    let err = pipeline
        .run_code_task(SoftwareCodeTask::exact_edit(
            &workspace,
            "Change answer but fail tests",
            "src/lib.rs",
            "1",
            "2",
            env::current_exe().unwrap(),
            vec!["--definitely-not-a-valid-test-flag".to_string()],
        ))
        .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
    let state = pipeline.kernel().state_snapshot().unwrap();
    assert!(
        !state
            .threads
            .values()
            .any(|thread| thread.role == "SupervisorAgent"
                && thread.status == ThreadStatus::Completed)
    );
    assert!(state.final_submissions.len() < 6);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn task_only_code_request_can_infer_single_safe_edit() {
    let workspace = temp_workspace("agent-os-conformance-inferred-edit");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    let spec = SoftwareCodeTask::plan_from_task(
        &workspace,
        "Change answer from one to two",
        None,
        env::current_exe().unwrap(),
        vec!["--help".to_string()],
    )
    .unwrap();
    assert_eq!(spec.edit_plan_source, SoftwareEditPlanSource::Inferred);
    assert_eq!(spec.file, PathBuf::from("src/lib.rs"));
    assert_eq!(spec.old, "1");
    assert_eq!(spec.new, "2");

    let pipeline = SoftwareEngineeringPipeline::new(Kernel::new()).unwrap();
    let report = pipeline.run_code_task(spec).unwrap();
    assert_eq!(report.status, ThreadStatus::Completed);
    assert_eq!(report.edit_plan_source, SoftwareEditPlanSource::Inferred);
    assert_eq!(
        fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> i32 { 2 }\n"
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn task_only_code_request_fails_closed_when_edit_is_ambiguous() {
    let workspace = temp_workspace("agent-os-conformance-ambiguous-edit");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn answer() -> i32 { 1 }\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/other.rs"),
        "pub fn other() -> i32 { 1 }\n",
    )
    .unwrap();
    let err = SoftwareCodeTask::plan_from_task(
        &workspace,
        "Change answer from one to two",
        None,
        env::current_exe().unwrap(),
        vec!["--help".to_string()],
    )
    .unwrap_err();
    assert!(matches!(err, AgentOsError::Validation(_)));
    let _ = fs::remove_dir_all(workspace);
}

fn temp_workspace(prefix: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        new_id("case_")
    ))
}
