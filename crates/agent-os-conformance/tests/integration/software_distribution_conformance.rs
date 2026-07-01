use crate::common::*;
use agent_os_sys::{PackageManifest, PackageType};
use agent_os_thread::{SoftwareCodeTask, SoftwareEditPlanSource, SoftwareWorkflowPrompt};
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
fn software_distribution_builds_prompt_workflow_without_scripted_pipeline() {
    let spec = SoftwareCodeTask::exact_edit(
        "workspace",
        "Change answer from one to two",
        "src/lib.rs",
        "1",
        "2",
        env::current_exe().unwrap(),
        vec!["--help".to_string()],
    );
    let prompt = SoftwareWorkflowPrompt::from_code_task(&spec).unwrap();

    assert_eq!(prompt.package_name, "software-engineering");
    assert_eq!(prompt.review_policy_name, "software-engineering-review");
    assert_eq!(
        prompt.final_answer_policy_name,
        "software-engineering-final-answer"
    );
    assert!(prompt
        .workflow_steps
        .iter()
        .any(|step| step.label == "review" && step.core_role == "ReviewerAgent"));
    assert!(prompt.prompt.contains("Flexible Workflow Policy"));
    assert!(prompt.prompt.contains(
        "Do not assume a fixed Explorer -> Coder -> Tester -> Reviewer -> Verifier sequence"
    ));
    assert!(prompt.prompt.contains("submit_final as the last action"));
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

    let prompt = SoftwareWorkflowPrompt::from_code_task(&spec).unwrap();
    assert!(prompt.prompt.contains("Edit plan source: Inferred"));
    assert!(prompt.prompt.contains("Target file:"));
    assert!(prompt.prompt.contains("lib.rs"));
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
