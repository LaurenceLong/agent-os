use super::distro::SoftwareEngineeringDistro;
use super::types::{
    SoftwareCodeTask, SoftwareWorkflowPrompt, SoftwareWorkflowRequest, SoftwareWorkflowStep,
};
use agent_os_sys::{AgentOsError, AgentOsResult};
use serde_json::Value;

impl SoftwareWorkflowPrompt {
    pub fn from_code_task(spec: &SoftwareCodeTask) -> AgentOsResult<Self> {
        Self::from_request(&SoftwareWorkflowRequest::from_code_task(spec))
    }

    pub fn from_request(request: &SoftwareWorkflowRequest) -> AgentOsResult<Self> {
        let distro = SoftwareEngineeringDistro::load_default()?;
        build_prompt(request, &distro)
    }
}

fn build_prompt(
    request: &SoftwareWorkflowRequest,
    distro: &SoftwareEngineeringDistro,
) -> AgentOsResult<SoftwareWorkflowPrompt> {
    let review_policy_name = required_string(&distro.review_policy, "policy_name")?;
    let final_answer_policy_name = required_string(&distro.final_answer_policy, "policy_name")?;
    let acceptance_criteria =
        required_string_array(&distro.final_answer_policy, "acceptance_criteria")?;
    let required_evidence_types =
        required_string_array(&distro.final_answer_policy, "required_evidence_types")?;
    let workflow_steps = workflow_steps();
    let prompt = render_prompt(
        request,
        distro,
        &workflow_steps,
        &acceptance_criteria,
        &required_evidence_types,
        review_policy_name,
        final_answer_policy_name,
    );

    Ok(SoftwareWorkflowPrompt {
        package_name: distro.manifest.package_name.clone(),
        prompt,
        workflow_steps,
        acceptance_criteria,
        review_policy_name: review_policy_name.to_string(),
        final_answer_policy_name: final_answer_policy_name.to_string(),
        required_evidence_types,
    })
}

fn workflow_steps() -> Vec<SoftwareWorkflowStep> {
    vec![
        SoftwareWorkflowStep {
            label: "explore".to_string(),
            core_role: "WorkerAgent".to_string(),
            objective: "Inspect only the context needed to make a scoped plan.".to_string(),
        },
        SoftwareWorkflowStep {
            label: "implement".to_string(),
            core_role: "WorkerAgent".to_string(),
            objective: "Apply the smallest coherent workspace change through apply_patch."
                .to_string(),
        },
        SoftwareWorkflowStep {
            label: "validate".to_string(),
            core_role: "WorkerAgent".to_string(),
            objective: "Run focused commands and attach command evidence.".to_string(),
        },
        SoftwareWorkflowStep {
            label: "review".to_string(),
            core_role: "ReviewerAgent".to_string(),
            objective: "Review exact artifacts, risks, and evidence quality when the Supervisor chooses an independent review step.".to_string(),
        },
        SoftwareWorkflowStep {
            label: "finalize".to_string(),
            core_role: "SupervisorAgent".to_string(),
            objective: "Submit an evidence-backed final answer only after the chosen workflow has satisfied the final policy.".to_string(),
        },
    ]
}

fn render_prompt(
    request: &SoftwareWorkflowRequest,
    distro: &SoftwareEngineeringDistro,
    workflow_steps: &[SoftwareWorkflowStep],
    acceptance_criteria: &[String],
    required_evidence_types: &[String],
    review_policy_name: &str,
    final_answer_policy_name: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push("# Software Engineering Workflow".to_string());
    lines.push(String::new());
    lines.push(format!("Task: {}", request.task));
    lines.push(format!(
        "Workspace root: {}",
        request.workspace_root.to_string_lossy()
    ));
    if let Some(file) = &request.target_file {
        lines.push(format!("Target file: {}", file.to_string_lossy()));
    }
    if let Some(source) = request.edit_plan_source {
        lines.push(format!("Edit plan source: {source:?}"));
    }
    if let Some(edit) = &request.exact_edit {
        lines.push("Exact edit request: replace the old text with the new text.".to_string());
        lines.push("Old text:".to_string());
        lines.push(edit.old.clone());
        lines.push("New text:".to_string());
        lines.push(edit.new.clone());
    } else {
        lines.push(
            "No exact edit was provided. Infer the implementation plan from the task and current workspace evidence.".to_string(),
        );
    }
    lines.push(format!(
        "Validation command: {}",
        validation_command(request)
    ));
    lines.push(String::new());
    lines.push("## Distribution Prompts".to_string());
    lines.push(String::new());
    lines.push("### SupervisorAgent".to_string());
    lines.push(distro.supervisor_prompt.trim().to_string());
    lines.push(String::new());
    lines.push("### WorkerAgent".to_string());
    lines.push(distro.worker_prompt.trim().to_string());
    lines.push(String::new());
    lines.push("### ReviewerAgent".to_string());
    lines.push(distro.reviewer_prompt.trim().to_string());
    lines.push(String::new());
    lines.push("## Flexible Workflow Policy".to_string());
    lines.push(String::new());
    lines.push(
        "The Supervisor decides the workflow at runtime. Do not assume a fixed Explorer -> Coder -> Tester -> Reviewer -> Verifier sequence.".to_string(),
    );
    lines.push(
        "Use workflow labels only as prompt-level planning concepts; every delegated action must still map to a core Agent-OS role.".to_string(),
    );
    lines.push(
        "A simple low-risk edit may use a compact explore/implement/validate/finalize path. A riskier change may add review, revision, or parallel workers.".to_string(),
    );
    lines.push(
        "Use apply_patch for workspace mutations, run_command for validation, record evidence for claims, and submit_final as the last action.".to_string(),
    );
    lines.push(String::new());
    lines.push("Available workflow labels:".to_string());
    for step in workflow_steps {
        lines.push(format!(
            "- {} -> {}: {}",
            step.label, step.core_role, step.objective
        ));
    }
    lines.push(String::new());
    lines.push("## Policy Packs".to_string());
    lines.push(String::new());
    lines.push(format!("Review policy: {review_policy_name}"));
    lines.push(format!("Final answer policy: {final_answer_policy_name}"));
    lines.push("Acceptance criteria:".to_string());
    for criterion in acceptance_criteria {
        lines.push(format!("- {criterion}"));
    }
    lines.push("Required evidence types:".to_string());
    for evidence_type in required_evidence_types {
        lines.push(format!("- {evidence_type}"));
    }
    lines.join("\n")
}

fn validation_command(request: &SoftwareWorkflowRequest) -> String {
    let mut command = request.test_program.to_string_lossy().to_string();
    for arg in &request.test_args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

fn required_string<'a>(object: &'a Value, field: &str) -> AgentOsResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| AgentOsError::Validation(format!("software policy omitted {field}")))
}

fn required_string_array(object: &Value, field: &str) -> AgentOsResult<Vec<String>> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AgentOsError::Validation(format!("software policy omitted {field}")))?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                AgentOsError::Validation(format!("software policy {field} entries must be strings"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn software_workflow_prompt_uses_distribution_policy_without_fixed_pipeline() {
        let spec = SoftwareCodeTask::exact_edit(
            "workspace",
            "Change answer from one to two",
            "src/lib.rs",
            "1",
            "2",
            "cargo",
            vec!["test".to_string()],
        );

        let prompt = SoftwareWorkflowPrompt::from_code_task(&spec).unwrap();

        assert_eq!(prompt.package_name, "software-engineering");
        assert!(prompt
            .workflow_steps
            .iter()
            .any(|step| step.core_role == "SupervisorAgent"));
        assert!(prompt.prompt.contains("Flexible Workflow Policy"));
        assert!(prompt.prompt.contains("Target file: src/lib.rs"));
        assert!(prompt.prompt.contains("Validation command: cargo test"));
        assert!(prompt.prompt.contains("Use workflow labels only"));
        assert!(!prompt.prompt.contains("Supervisor -> Explorer -> Coder"));
    }

    #[test]
    fn software_workflow_prompt_allows_task_inferred_planning() {
        let request = SoftwareWorkflowRequest {
            workspace_root: PathBuf::from("workspace"),
            task: "Improve the parser".to_string(),
            target_file: None,
            exact_edit: None,
            test_program: PathBuf::from("cargo"),
            test_args: vec!["test".to_string(), "-p".to_string(), "parser".to_string()],
            edit_plan_source: None,
        };

        let prompt = SoftwareWorkflowPrompt::from_request(&request).unwrap();

        assert!(prompt.prompt.contains("No exact edit was provided"));
        assert!(prompt
            .prompt
            .contains("Validation command: cargo test -p parser"));
    }
}
