use crate::args::RunOptions;
use crate::support::{
    ensure_safe_relative_workspace_path, first_evidence_id, io_result, open_kernel,
    task_output_content, write_task_bundle_if_requested,
};
use agent_os_kernel::{
    CommitArtifactInput, CompleteTaskInput, Kernel, RegisterGoalInput, SpawnAgentInput,
    SpawnTaskInput, ToolInvokeInput, UpdateTaskInput,
};
use agent_os_sys::*;
use agent_os_thread::{ExternalProcessModelClient, RuntimeConfig, ThreadRuntime};
use serde_json::{json, Value};
use std::env;
use std::fs;

pub(crate) fn run_e2e_task(options: &RunOptions) -> AgentOsResult<Value> {
    ensure_safe_relative_workspace_path(&options.output, "--output")?;
    if let Some(bundle_output) = &options.bundle_output {
        ensure_safe_relative_workspace_path(bundle_output, "--bundle-output")?;
    }
    io_result(
        fs::create_dir_all(&options.workspace),
        "create workspace directory",
    )?;
    if options.model_command.is_some() {
        return run_external_model_task(options);
    }
    let output_path = options.workspace.join(&options.output);
    let before = fs::read_to_string(&output_path).ok();
    let content = task_output_content(&options.task);

    let kernel = open_kernel(&options.state_db)?;
    let goal = kernel.register_goal(RegisterGoalInput {
        namespace: "cli".to_string(),
        created_by: "agent-os-cli".to_string(),
        title: options.task.clone(),
        description: options.task.clone(),
        acceptance_criteria: vec!["workspace output file is written".to_string()],
        constraints: Vec::new(),
        risk_level: 3,
        deadline: None,
    })?;
    let task = kernel.spawn_task(SpawnTaskInput {
        goal_id: goal.goal_id.clone(),
        parent_task_id: None,
        title: "Complete user workspace task".to_string(),
        description: options.task.clone(),
        depends_on: Vec::new(),
        required_artifact_types: vec![ArtifactType::Patch],
        required_evidence_types: vec![EvidenceType::DiffRef, EvidenceType::CommandLog],
        priority: 10,
        risk_level: 3,
    })?;
    let agent = kernel.spawn_agent(SpawnAgentInput {
        task_id: task.task_id.clone(),
        role_profile_id: "role_worker".to_string(),
        owner: "agent-os-cli".to_string(),
        local_goal: options.task.clone(),
        success_criteria: vec!["workspace output file is written".to_string()],
        failure_criteria: Vec::new(),
        parent_thread_id: None,
        workspace_roots: vec![options.workspace.to_string_lossy().to_string()],
    })?;
    kernel.update_task(UpdateTaskInput {
        task_id: task.task_id.clone(),
        status: Some(TaskStatus::Running),
        blocked_reason: None,
        owner_agent_id: Some(agent.agent_id.clone()),
        title: None,
        description: None,
        checklist: None,
    })?;
    let turn = kernel.start_turn(&agent.thread_id)?;
    let stream = kernel.open_stream_session(StreamRequest {
        thread_id: agent.thread_id.clone(),
        turn_id: turn.active_turn.turn_id.clone(),
        provider_profile_id: agent.config_snapshot.provider_profile_id.clone(),
        model_routing_policy_id: agent.config_snapshot.model_routing_policy_id.clone(),
        requested_model_alias: None,
        role: agent.role.clone(),
        task_id: task.task_id.clone(),
        reasoning_profile: agent.config_snapshot.reasoning_profile.clone(),
        tool_visibility_profile: None,
        output_schema: None,
    })?;
    kernel.record_provider_usage(
        &stream.session_id,
        ProviderUsage {
            input_tokens: options.task.len() as u64,
            output_tokens: content.len() as u64,
            cost: 0.0,
        },
    )?;
    kernel.complete_stream_session(&stream.session_id)?;

    let env = kernel.create_environment(
        BackendType::IsolatedWorktree,
        options.workspace.to_string_lossy(),
        "sbox_workspace_write",
        ReusePolicy::TaskScoped,
    )?;
    let environment_lease = kernel.attach_environment(
        &env.environment_id,
        &agent.agent_id,
        &agent.thread_id,
        &task.task_id,
        AttachMode::WorkspaceWrite,
    )?;

    let tool_capability = kernel.grant_capability(
        &agent.agent_id,
        &task.task_id,
        vec!["tool.invoke".to_string()],
        vec!["tool:*".to_string()],
        4,
        None,
    )?;
    let write_invocation = kernel.invoke_tool(
        &agent.agent_id,
        &task.task_id,
        &agent.session_id,
        tool_capability.capability_id.clone(),
        4,
        ToolInvokeInput {
            tool_name: "write_file".to_string(),
            input: json!({
                "workspace_root": options.workspace.to_string_lossy(),
                "path": options.output.to_string_lossy(),
                "content": content,
            }),
            evidence_claim: Some("workspace output file was written".to_string()),
        },
    )?;
    let diff_evidence_id = first_evidence_id(&write_invocation)?;
    let written_path = write_invocation
        .output
        .as_ref()
        .and_then(|output| output.get("written_path"))
        .and_then(Value::as_str)
        .ok_or_else(|| AgentOsError::Validation("write_file omitted written_path".to_string()))?
        .to_string();
    let command_program = env::current_exe().map_err(|error| {
        AgentOsError::Validation(format!("resolve current executable: {error}"))
    })?;
    let command_invocation = kernel.invoke_tool(
        &agent.agent_id,
        &task.task_id,
        &agent.session_id,
        tool_capability.capability_id,
        4,
        ToolInvokeInput {
            tool_name: "run_command".to_string(),
            input: json!({
                "program": command_program.to_string_lossy(),
                "args": ["--help"],
                "cwd": options.workspace.to_string_lossy(),
            }),
            evidence_claim: Some("agent-os CLI process tool executed".to_string()),
        },
    )?;
    let command_evidence_id = first_evidence_id(&command_invocation)?;
    let artifact = kernel.commit_artifact(CommitArtifactInput {
        goal_id: goal.goal_id.clone(),
        task_id: task.task_id.clone(),
        owner_agent_id: agent.agent_id.clone(),
        artifact_type: ArtifactType::Patch,
        blob_ref: Some(written_path.clone()),
        content_hash: None,
        inline_bytes: None,
        metadata: json!({
            "output_path": written_path,
            "before": before,
            "environment_id": env.environment_id,
            "environment_lease_id": environment_lease.environment_lease_id,
        }),
        evidence_ids: vec![diff_evidence_id.clone()],
        supersedes: None,
    })?;
    kernel.complete_task(CompleteTaskInput {
        task_id: task.task_id.clone(),
        artifact_ids: vec![artifact.artifact_id.clone()],
        evidence_ids: vec![diff_evidence_id.clone(), command_evidence_id.clone()],
    })?;
    kernel.submit_final(
        &agent.agent_id,
        &task.task_id,
        FinalSubmission {
            summary: format!("Completed task and wrote {}", output_path.to_string_lossy()),
            changed_artifacts: vec![artifact.artifact_id.clone()],
            evidence_map: vec![
                EvidenceMapEntry {
                    claim: "workspace output file was written".to_string(),
                    evidence_refs: vec![diff_evidence_id.clone()],
                },
                EvidenceMapEntry {
                    claim: "task was executed through the Agent-OS CLI run loop".to_string(),
                    evidence_refs: vec![command_evidence_id.clone()],
                },
            ],
            unverified_claims: Vec::new(),
            known_risks: Vec::new(),
            tests_run: vec!["agent-os e2e run loop".to_string()],
            tests_not_run: Vec::new(),
            approvals: Vec::new(),
        },
    )?;
    kernel.transition_thread(
        &agent.thread_id,
        ThreadStatus::Completed,
        Some("CLI e2e task completed".to_string()),
    )?;
    let bundle_path = write_task_bundle_if_requested(
        &kernel,
        &task.task_id,
        &options.workspace,
        &options.bundle_output,
    )?;

    let replayed = Kernel::from_events(&kernel.events()?)?;
    let replayed_state = replayed.state_snapshot()?;
    Ok(json!({
        "status": "completed",
        "goal_id": goal.goal_id,
        "task_id": task.task_id,
        "thread_id": agent.thread_id,
        "agent_id": agent.agent_id,
        "output_path": output_path.to_string_lossy(),
        "state_db": options.state_db.as_ref().map(|path| path.to_string_lossy().to_string()),
        "artifact_id": artifact.artifact_id,
        "bundle_path": bundle_path,
        "evidence_ids": [
            diff_evidence_id,
            command_evidence_id
        ],
        "provider_stream_session_id": stream.session_id,
        "events": kernel.events()?.len(),
        "replay": {
            "tasks": replayed_state.tasks.len(),
            "artifacts": replayed_state.artifacts.len(),
            "evidence": replayed_state.evidence.len(),
            "final_submissions": replayed_state.final_submissions.len()
        }
    }))
}

fn run_external_model_task(options: &RunOptions) -> AgentOsResult<Value> {
    let model_command = options.model_command.as_ref().ok_or_else(|| {
        AgentOsError::Validation("--model-command is required for external model run".to_string())
    })?;
    let output_path = options.workspace.join(&options.output);
    let task_prompt = format!(
        "{}\nRequested workspace output path: {}",
        options.task,
        options.output.to_string_lossy()
    );

    let kernel = open_kernel(&options.state_db)?;
    let goal = kernel.register_goal(RegisterGoalInput {
        namespace: "cli".to_string(),
        created_by: "agent-os-cli".to_string(),
        title: options.task.clone(),
        description: task_prompt.clone(),
        acceptance_criteria: vec!["external model completed the workspace task".to_string()],
        constraints: Vec::new(),
        risk_level: 4,
        deadline: None,
    })?;
    let task = kernel.spawn_task(SpawnTaskInput {
        goal_id: goal.goal_id.clone(),
        parent_task_id: None,
        title: "Complete user workspace task with external model".to_string(),
        description: task_prompt.clone(),
        depends_on: Vec::new(),
        required_artifact_types: vec![ArtifactType::Patch],
        required_evidence_types: vec![EvidenceType::DiffRef],
        priority: 10,
        risk_level: 4,
    })?;
    let agent = kernel.spawn_agent(SpawnAgentInput {
        task_id: task.task_id.clone(),
        role_profile_id: "role_worker".to_string(),
        owner: "agent-os-cli".to_string(),
        local_goal: task_prompt,
        success_criteria: vec!["external model submitted an evidence-backed final".to_string()],
        failure_criteria: Vec::new(),
        parent_thread_id: None,
        workspace_roots: vec![options.workspace.to_string_lossy().to_string()],
    })?;

    let client = ExternalProcessModelClient::new(model_command.clone(), options.model_args.clone());
    let mut runtime = ThreadRuntime::new(kernel.clone(), agent.thread_id.clone(), client);
    let report = runtime.run_to_completion(RuntimeConfig {
        workspace_root: options.workspace.clone(),
        attach_mode: AttachMode::WorkspaceWrite,
        max_steps: 16,
        requested_model_alias: None,
        tool_risk_ceiling: 4,
        auto_commit_patch_artifacts: true,
        fail_on_process_nonzero: true,
    })?;
    let artifact_ids: Vec<_> = report
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect();
    let evidence_ids: Vec<_> = report
        .tool_results
        .iter()
        .flat_map(|result| result.evidence_ids.clone())
        .collect();
    let bundle_path = write_task_bundle_if_requested(
        &kernel,
        &task.task_id,
        &options.workspace,
        &options.bundle_output,
    )?;
    let replayed = Kernel::from_events(&kernel.events()?)?;
    let replayed_state = replayed.state_snapshot()?;
    Ok(json!({
        "status": "completed",
        "goal_id": goal.goal_id,
        "task_id": task.task_id,
        "thread_id": report.thread_id,
        "agent_id": agent.agent_id,
        "output_path": output_path.to_string_lossy(),
        "state_db": options.state_db.as_ref().map(|path| path.to_string_lossy().to_string()),
        "model_command": model_command.to_string_lossy(),
        "model_args": &options.model_args,
        "bundle_path": bundle_path,
        "runtime_status": report.status,
        "artifact_ids": artifact_ids,
        "evidence_ids": evidence_ids,
        "provider_stream_session_ids": report.provider_stream_session_ids,
        "tool_results": report.tool_results,
        "artifacts": report.artifacts,
        "events": report.events,
        "replay": {
            "tasks": replayed_state.tasks.len(),
            "artifacts": replayed_state.artifacts.len(),
            "evidence": replayed_state.evidence.len(),
            "final_submissions": replayed_state.final_submissions.len()
        }
    }))
}

#[cfg(test)]
mod tests;
