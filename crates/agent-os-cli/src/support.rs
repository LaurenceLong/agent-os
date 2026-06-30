use agent_os_kernel::Kernel;
use agent_os_store::LocalBlobStore;
use agent_os_store_sqlite::SqliteStore;
use agent_os_sys::{AgentOsError, AgentOsResult, ToolInvocation};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) fn ensure_safe_relative_workspace_path(path: &Path, flag: &str) -> AgentOsResult<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentOsError::Validation(format!(
            "{flag} must be a relative path inside --workspace"
        )));
    }
    Ok(())
}

pub(crate) fn task_output_content(task: &str) -> String {
    format!(
        "# Agent-OS Task Result\n\nStatus: completed\n\nTask:\n{task}\n\nEvidence:\n- Written by `agent-os run` through the kernel task lifecycle.\n"
    )
}

pub(crate) fn io_result<T>(result: io::Result<T>, context: &str) -> AgentOsResult<T> {
    result.map_err(|error| AgentOsError::Validation(format!("{context}: {error}")))
}

pub(crate) fn open_kernel(state_db: &Option<PathBuf>) -> AgentOsResult<Kernel> {
    let kernel = if let Some(path) = state_db {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                io_result(fs::create_dir_all(parent), "create state database parent")?;
            }
        }
        let store = SqliteStore::open(path)?;
        Kernel::with_replayed_store(store)?
    } else {
        Kernel::new()
    };
    attach_cli_blob_stores(kernel, state_db)
}

pub(crate) fn open_kernel_from_state_db(path: &Path) -> AgentOsResult<Kernel> {
    open_kernel(&Some(path.to_path_buf()))
}

fn attach_cli_blob_stores(kernel: Kernel, state_db: &Option<PathBuf>) -> AgentOsResult<Kernel> {
    let root = match state_db {
        Some(path) => match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::env::current_dir()
                .map_err(|error| AgentOsError::Validation(format!("resolve cwd: {error}")))?,
        },
        None => std::env::current_dir()
            .map_err(|error| AgentOsError::Validation(format!("resolve cwd: {error}")))?
            .join(".agent-os"),
    };
    let blob_root = root.join("blobs");
    let artifact_blobs = LocalBlobStore::new(blob_root.join("artifacts"))?;
    let evidence_blobs = LocalBlobStore::new(blob_root.join("evidence"))?;
    Ok(kernel.with_blob_stores(artifact_blobs, evidence_blobs))
}

pub(crate) fn write_task_bundle_if_requested(
    kernel: &Kernel,
    task_id: &str,
    workspace: &Path,
    bundle_output: &Option<PathBuf>,
) -> AgentOsResult<Option<String>> {
    let Some(relative_path) = bundle_output else {
        return Ok(None);
    };
    ensure_safe_relative_workspace_path(relative_path, "--bundle-output")?;
    let bundle = kernel.export_task_bundle(task_id)?;
    let path = workspace.join(relative_path);
    if let Some(parent) = path.parent() {
        io_result(fs::create_dir_all(parent), "create bundle output parent")?;
    }
    let bytes = serde_json::to_vec_pretty(&bundle)?;
    io_result(fs::write(&path, bytes), "write task bundle")?;
    Ok(Some(path.to_string_lossy().to_string()))
}

pub(crate) fn first_evidence_id(invocation: &ToolInvocation) -> AgentOsResult<String> {
    invocation.evidence_ids.first().cloned().ok_or_else(|| {
        AgentOsError::Validation(format!(
            "tool {} produced no evidence",
            invocation.tool_name
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_kernel::{
        AttachEvidenceInput, CommitArtifactInput, RegisterGoalInput, SpawnAgentInput,
        SpawnTaskInput,
    };
    use agent_os_sys::{ArtifactType, EvidenceType};
    use serde_json::json;

    #[test]
    fn opened_kernel_persists_inline_evidence_and_artifact_blobs() {
        let root =
            std::env::temp_dir().join(format!("agent-os-cli-blob-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let state_db = root.join("state.sqlite");

        let kernel = open_kernel(&Some(state_db)).unwrap();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "cli-test".to_string(),
                created_by: "test".to_string(),
                title: "Blob store".to_string(),
                description: "Blob store".to_string(),
                acceptance_criteria: vec!["inline blobs can be persisted".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id.clone(),
                parent_task_id: None,
                title: "Persist blobs".to_string(),
                description: "Persist blobs".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 1,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id.clone(),
                role_profile_id: "role_worker".to_string(),
                owner: "test".to_string(),
                goal: "Persist blobs".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![root.to_string_lossy().to_string()],
            })
            .unwrap();

        let evidence = kernel
            .attach_evidence(AttachEvidenceInput {
                goal_id: goal.goal_id.clone(),
                task_id: Some(task.task_id.clone()),
                artifact_id: None,
                evidence_type: EvidenceType::CommandLog,
                producer_agent_id: Some(agent.agent_id.clone()),
                claim: Some("inline command evidence".to_string()),
                blob_ref: None,
                content_hash: None,
                inline_bytes: Some(b"command output".to_vec()),
                metadata: json!({}),
            })
            .unwrap();
        assert!(evidence.blob_ref.is_some());

        let artifact = kernel
            .commit_artifact(CommitArtifactInput {
                goal_id: goal.goal_id,
                task_id: task.task_id,
                owner_agent_id: agent.agent_id,
                artifact_type: ArtifactType::BenchmarkResult,
                blob_ref: None,
                content_hash: None,
                inline_bytes: Some(b"benchmark result".to_vec()),
                metadata: json!({}),
                evidence_ids: vec![evidence.evidence_id],
                supersedes: None,
            })
            .unwrap();
        assert!(artifact.blob_ref.is_some());

        let _ = fs::remove_dir_all(root);
    }
}
