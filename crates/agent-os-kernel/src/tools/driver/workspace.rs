use crate::util::required_string;
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub(super) fn run_workspace_write_file(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let content = required_string(input, "content")?;
    let (root, written_path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, true)?;
    if let Some(parent) = written_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AgentOsError::Validation(format!("create parent directory: {error}"))
        })?;
    }
    fs::write(&written_path, content.as_bytes())
        .map_err(|error| AgentOsError::Validation(format!("write workspace file: {error}")))?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "written_path": written_path.to_string_lossy(),
        "bytes_written": content.len(),
    }))
}

pub(super) fn run_workspace_read_file(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, false)?;
    let content = fs::read_to_string(&path)
        .map_err(|error| AgentOsError::Validation(format!("read workspace file: {error}")))?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "path": path.to_string_lossy(),
        "content": content,
        "bytes_read": content.len(),
    }))
}

pub(super) fn run_workspace_delete_file(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, true)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace file: {error}")))?;
    if !metadata.is_file() {
        return Err(AgentOsError::Validation(
            "delete_file only deletes files".to_string(),
        ));
    }
    let deleted_bytes = metadata.len();
    fs::remove_file(&path)
        .map_err(|error| AgentOsError::Validation(format!("delete workspace file: {error}")))?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "deleted_path": path.to_string_lossy(),
        "deleted_bytes": deleted_bytes,
    }))
}

pub(super) fn run_workspace_replace_text(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let old = required_string(input, "old")?;
    let new = required_string(input, "new")?;
    if old.is_empty() {
        return Err(AgentOsError::Validation(
            "replace_text old text must not be empty".to_string(),
        ));
    }
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, true)?;
    let before = fs::read_to_string(&path)
        .map_err(|error| AgentOsError::Validation(format!("read workspace file: {error}")))?;
    let occurrences = before.matches(&old).count();
    if occurrences != 1 {
        return Err(AgentOsError::Validation(format!(
            "replace_text expected exactly one match, found {occurrences}"
        )));
    }
    let after = before.replacen(&old, &new, 1);
    fs::write(&path, after.as_bytes())
        .map_err(|error| AgentOsError::Validation(format!("write workspace file: {error}")))?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "changed_path": path.to_string_lossy(),
        "replacements": 1,
        "before": before,
        "after": after,
    }))
}

pub(super) fn run_process(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let program = required_string(input, "program")?;
    let cwd = PathBuf::from(required_string(input, "cwd")?);
    let args = input
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentOsError::Validation("run_command args must be an array".to_string()))?
        .iter()
        .map(|arg| {
            arg.as_str().map(str::to_string).ok_or_else(|| {
                AgentOsError::Validation("run_command args must be strings".to_string())
            })
        })
        .collect::<AgentOsResult<Vec<_>>>()?;
    let env = optional_string_map(input, "env")?;
    let cwd = canonical_workspace_root(&cwd)?;
    ensure_environment_lease_for_path(kernel, syscall, &cwd, false)?;
    let mut command = Command::new(program);
    command.args(args).current_dir(&cwd);
    if !env.is_empty() {
        command.envs(env);
    }
    let output = command
        .output()
        .map_err(|error| AgentOsError::Validation(format!("run process: {error}")))?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    }))
}

fn optional_string_map(input: &Value, field: &str) -> AgentOsResult<BTreeMap<String, String>> {
    let Some(value) = input.get(field) else {
        return Ok(BTreeMap::new());
    };
    let map = value.as_object().ok_or_else(|| {
        AgentOsError::Validation(format!("run_command {field} must be an object"))
    })?;
    let mut env = BTreeMap::new();
    for (key, value) in map {
        if key.is_empty() {
            return Err(AgentOsError::Validation(
                "run_command env keys must not be empty".to_string(),
            ));
        }
        let value = value.as_str().ok_or_else(|| {
            AgentOsError::Validation("run_command env values must be strings".to_string())
        })?;
        env.insert(key.clone(), value.to_string());
    }
    Ok(env)
}

fn ensure_safe_relative_path(path: &Path) -> AgentOsResult<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentOsError::Validation(
            "workspace path must be relative and stay inside workspace".to_string(),
        ));
    }
    Ok(())
}

fn resolve_workspace_path(
    workspace_root: &Path,
    relative_path: &Path,
) -> AgentOsResult<(PathBuf, PathBuf)> {
    ensure_safe_relative_path(relative_path)?;
    let root = canonical_workspace_root(workspace_root)?;
    Ok((root.clone(), root.join(relative_path)))
}

fn canonical_workspace_root(path: &Path) -> AgentOsResult<PathBuf> {
    fs::create_dir_all(path)
        .map_err(|error| AgentOsError::Validation(format!("create workspace root: {error}")))?;
    path.canonicalize()
        .map_err(|error| AgentOsError::Validation(format!("canonicalize workspace root: {error}")))
}

fn ensure_environment_lease_for_path(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    workspace_root: &Path,
    require_write: bool,
) -> AgentOsResult<()> {
    let acb = kernel
        .thread_by_agent(&syscall.agent_id)?
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
    let state = kernel.read_state()?;
    for lease in state.environment_leases.values() {
        if lease.agent_id != syscall.agent_id
            || lease.thread_id != acb.thread_id
            || lease.task_id != syscall.task_id
            || lease.status != EnvironmentLeaseStatus::Active
        {
            continue;
        }
        if require_write
            && !matches!(
                lease.attach_mode,
                AttachMode::WorkspaceWrite | AttachMode::Exclusive
            )
        {
            continue;
        }
        let Some(env) = state.environments.get(&lease.environment_id) else {
            continue;
        };
        if !environment_matches_workspace(env, workspace_root) {
            continue;
        }
        let Some(sandbox) = state.sandbox_profiles.get(&env.sandbox_profile_id) else {
            continue;
        };
        if require_write && !sandbox_allows_workspace_write(sandbox) {
            continue;
        }
        return Ok(());
    }
    Err(AgentOsError::PermissionDenied(
        "tool requires an active compatible environment lease".to_string(),
    ))
}

fn environment_matches_workspace(env: &ExecutionEnvironment, workspace_root: &Path) -> bool {
    PathBuf::from(&env.template_name)
        .canonicalize()
        .is_ok_and(|env_root| env_root == workspace_root)
}

fn sandbox_allows_workspace_write(sandbox: &SandboxProfile) -> bool {
    sandbox.status == ProfileStatus::Active
        && matches!(
            sandbox.filesystem_mode,
            FilesystemMode::WorkspaceWrite | FilesystemMode::IsolatedWorktree
        )
}
