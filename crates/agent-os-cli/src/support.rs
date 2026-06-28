use agent_os_kernel::Kernel;
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
    let Some(path) = state_db else {
        return Ok(Kernel::new());
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            io_result(fs::create_dir_all(parent), "create state database parent")?;
        }
    }
    let store = SqliteStore::open(path)?;
    Kernel::with_replayed_store(store)
}

pub(crate) fn open_kernel_from_state_db(path: &Path) -> AgentOsResult<Kernel> {
    open_kernel(&Some(path.to_path_buf()))
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
