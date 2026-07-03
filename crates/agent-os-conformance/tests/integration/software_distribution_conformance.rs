use crate::common::*;
use agent_os_config::AgentOsPaths;
use agent_os_distro::{SoftwareCodeTask, SoftwareEditPlanSource, SoftwareWorkflowPrompt};
use agent_os_ecosystem::EcosystemDiscoverOptions;
use agent_os_sys::{PackageManifest, PackageType};
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
    assert!(root.join("prompts/producer.md").exists());
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
fn ecosystem_discovers_agent_os_package_manifest_contract() {
    let workspace = temp_workspace("agent-os-conformance-package-manifest");
    let home = workspace.join("home");
    let project = workspace.join("project");
    fs::create_dir_all(home.join("config")).unwrap();
    fs::create_dir_all(project.join(".agent-os/prompts")).unwrap();
    fs::create_dir_all(project.join(".agent-os/policy")).unwrap();
    fs::write(
        project.join(".agent-os/prompts/supervisor.md"),
        "Package prompt\n",
    )
    .unwrap();
    fs::write(project.join(".agent-os/policy/review.json"), "{}\n").unwrap();
    fs::write(
        project.join(".agent-os/manifest.json"),
        r#"{
  "manifest_version": "0.1",
  "package_name": "project-agent-package",
  "package_type": "agent",
  "version": "0.1.0",
  "entrypoint": "prompts/supervisor.md",
  "required_kernel_version": "0.3",
  "capabilities_requested": ["tool.invoke"],
  "roles_provided": ["ProducerAgent"],
  "tools_provided": [],
  "schemas": ["policy/review.json"],
  "signature": null
}
"#,
    )
    .unwrap();

    let catalog = agent_os_ecosystem::discover_ecosystem(&EcosystemDiscoverOptions {
        workspace_root: project.clone(),
        paths: AgentOsPaths {
            home: home.clone(),
            config_dir: home.join("config"),
            data_dir: home.join("data"),
            state_dir: home.join("state"),
            cache_dir: home.join("cache"),
            log_dir: home.join("log"),
            bin_dir: home.join("cache/bin"),
        },
    })
    .unwrap();

    assert_eq!(catalog.package_manifests.len(), 1);
    let package = &catalog.package_manifests[0];
    assert_eq!(package.manifest.package_name, "project-agent-package");
    assert_eq!(package.manifest.package_type, PackageType::Agent);
    assert!(package.manifest_path.ends_with("manifest.json"));
    assert!(package.root_path.ends_with(".agent-os"));
    assert!(!package.package_id.is_empty());
    assert!(!package.content_hash.is_empty());

    let _ = fs::remove_dir_all(workspace);
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
    assert!(prompt
        .prompt
        .contains("Do not assume a fixed Explorer -> Coder -> Tester -> Reviewer sequence"));
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
