use crate::process::StartProcessSessionInput;
use crate::state::ToolStreamOutput;
use crate::util::{hash_json, required_string};
use crate::*;
use agent_os_sys::*;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const PREVIEW_CHARS: usize = 2_000;

pub(in crate::tools) fn run_workspace_apply_patch(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let patch = required_string(input, "patch")?;
    let operation = parse_apply_patch(&patch)?;
    let relative_path = operation.path().to_path_buf();
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, true)?;
    ensure_workspace_target_contained(&root, &path)?;
    match operation {
        WorkspacePatch::Add { content, .. } => {
            if path.exists() {
                return Err(AgentOsError::Validation(format!(
                    "apply_patch add file target already exists: {}",
                    relative_path.display()
                )));
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AgentOsError::Validation(format!("create parent directory: {error}"))
                })?;
            }
            fs::write(&path, content.as_bytes()).map_err(|error| {
                AgentOsError::Validation(format!("write workspace file: {error}"))
            })?;
            let after_hash = hash_json(&content)?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "input": input.clone(),
                "driver_class": descriptor.driver_class,
                "operation": "create",
                "path": relative_path.to_string_lossy(),
                "created_path": path.to_string_lossy(),
                "bytes_written": content.len(),
                "after_hash": after_hash,
                "preview": preview_text(&content),
                "truncated": content.len() > PREVIEW_CHARS,
            }))
        }
        WorkspacePatch::Update { hunks, .. } => {
            let before = fs::read_to_string(&path).map_err(|error| {
                AgentOsError::Validation(format!("read workspace file: {error}"))
            })?;
            let after = apply_update_hunks(&before, &hunks)?;
            let before_hash = hash_json(&before)?;
            let after_hash = hash_json(&after)?;
            fs::write(&path, after.as_bytes()).map_err(|error| {
                AgentOsError::Validation(format!("write workspace file: {error}"))
            })?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "input": input.clone(),
                "driver_class": descriptor.driver_class,
                "operation": "update",
                "path": relative_path.to_string_lossy(),
                "changed_path": path.to_string_lossy(),
                "replacements": hunks.len(),
                "before_hash": before_hash,
                "after_hash": after_hash,
                "preview": preview_text(&after),
                "truncated": after.len() > PREVIEW_CHARS,
            }))
        }
        WorkspacePatch::Delete { .. } => {
            let metadata = fs::metadata(&path).map_err(|error| {
                AgentOsError::Validation(format!("stat workspace file: {error}"))
            })?;
            if !metadata.is_file() {
                return Err(AgentOsError::Validation(
                    "apply_patch delete operation only deletes files".to_string(),
                ));
            }
            let before = fs::read_to_string(&path).map_err(|error| {
                AgentOsError::Validation(format!("read workspace file: {error}"))
            })?;
            let before_hash = hash_json(&before)?;
            let deleted_bytes = metadata.len();
            fs::remove_file(&path).map_err(|error| {
                AgentOsError::Validation(format!("delete workspace file: {error}"))
            })?;
            Ok(json!({
                "tool": descriptor.name.clone(),
                "status": "ok",
                "input": input.clone(),
                "driver_class": descriptor.driver_class,
                "operation": "delete",
                "path": relative_path.to_string_lossy(),
                "deleted_path": path.to_string_lossy(),
                "deleted_bytes": deleted_bytes,
                "before_hash": before_hash,
                "preview": preview_text(&before),
                "truncated": before.len() > PREVIEW_CHARS,
            }))
        }
    }
}

pub(in crate::tools) fn run_workspace_read_file(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, false)?;
    ensure_workspace_target_contained(&root, &path)?;
    let (offset, limit) = super::super::builtin::read_file::parse_paging(input)?;
    let content = fs::read_to_string(&path)
        .map_err(|error| AgentOsError::Validation(format!("read workspace file: {error}")))?;
    let page = super::super::builtin::read_file::paginate_text(&content, offset, limit);
    let bytes_read = page.content.len();
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "path": path.to_string_lossy(),
        "content": page.content,
        "bytes_read": bytes_read,
        "offset": offset,
        "limit": limit,
        "total_lines": page.total_lines,
        "returned_lines": page.returned_lines,
        "next_offset": page.next_offset,
        "truncated": page.truncated,
        "omitted_lines": page.omitted_lines,
    }))
}

pub(in crate::tools) fn run_workspace_read_image(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let relative_path = PathBuf::from(required_string(input, "path")?);
    let mime_type = image_mime_type(&relative_path).ok_or_else(|| {
        AgentOsError::Validation(
            "read_image supports png, jpg, jpeg, gif, webp, bmp, tif, tiff, avif, and ico files"
                .to_string(),
        )
    })?;
    let (root, path) = resolve_workspace_path(&workspace_root, &relative_path)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, false)?;
    ensure_workspace_target_contained(&root, &path)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace image: {error}")))?;
    if !metadata.is_file() {
        return Err(AgentOsError::Validation(
            "read_image path must point to a file".to_string(),
        ));
    }
    if metadata.len() == 0 {
        return Err(AgentOsError::Validation(
            "read_image cannot read an empty image file".to_string(),
        ));
    }
    if metadata.len() > super::super::builtin::read_image::MAX_IMAGE_BYTES {
        return Err(AgentOsError::Validation(format!(
            "read_image file exceeds {} byte limit",
            super::super::builtin::read_image::MAX_IMAGE_BYTES
        )));
    }
    let bytes = fs::read(&path)
        .map_err(|error| AgentOsError::Validation(format!("read workspace image: {error}")))?;
    let encoded = BASE64_STANDARD.encode(&bytes);
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "path": path.to_string_lossy(),
        "mime_type": mime_type,
        "encoding": "base64",
        "data_url": format!("data:{mime_type};base64,{encoded}"),
        "bytes_read": bytes.len(),
    }))
}

pub(in crate::tools) fn run_workspace_glob_files(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let request = super::workspace_discovery::parse_glob_request(input)?;
    let relative_scope = PathBuf::from(request.path.as_deref().unwrap_or("."));
    let (root, scope) = resolve_workspace_path(&workspace_root, &relative_scope)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, false)?;
    ensure_workspace_target_contained(&root, &scope)?;
    let metadata = fs::metadata(&scope)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace glob path: {error}")))?;
    super::workspace_discovery::run_glob(descriptor, input, &root, &scope, metadata, request)
}

pub(in crate::tools) fn run_workspace_grep_files(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    input: &Value,
) -> AgentOsResult<Value> {
    let workspace_root = PathBuf::from(required_string(input, "workspace_root")?);
    let request = super::workspace_discovery::parse_grep_request(input)?;
    let relative_scope = PathBuf::from(request.path.as_deref().unwrap_or("."));
    let (root, scope) = resolve_workspace_path(&workspace_root, &relative_scope)?;
    ensure_environment_lease_for_path(kernel, syscall, &root, false)?;
    ensure_workspace_target_contained(&root, &scope)?;
    let metadata = fs::metadata(&scope)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace grep path: {error}")))?;
    super::workspace_discovery::run_grep(descriptor, input, &root, &scope, metadata, request)
}

pub(in crate::tools) fn run_process(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    let command_text = required_string(input, "command")?;
    let cwd = PathBuf::from(required_string(input, "cwd")?);
    let args = optional_string_array(input, "args")?.unwrap_or_default();
    let mode = run_command_mode(input, !args.is_empty())?;
    if mode == "shell" && !args.is_empty() {
        return Err(AgentOsError::Validation(
            "run_command args require exec mode".to_string(),
        ));
    }
    let env = optional_string_map(input, "env")?;
    let cwd = canonical_workspace_root(&cwd)?;
    ensure_environment_lease_for_path(kernel, syscall, &cwd, false)?;
    let environment_keys = env.keys().cloned().collect::<Vec<_>>();
    let (program, command_args) = run_command_program_and_args(&mode, &command_text, args.clone());
    let (stdout_spool_path, stderr_spool_path) = tool_output_spool_paths(tool_call_id)?;
    let acb = kernel
        .thread_by_agent(&syscall.agent_id)?
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", syscall.agent_id)))?;
    let process = kernel.start_process_session(StartProcessSessionInput {
        tool_call_id: tool_call_id.to_string(),
        agent_id: syscall.agent_id.clone(),
        thread_id: acb.thread_id,
        task_id: syscall.task_id.clone(),
        session_id: syscall.session_id.clone(),
        syscall_id: syscall.syscall_id.clone(),
        capability_id: syscall.capability_token.clone(),
        workspace_root: cwd.to_string_lossy().into_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        command_mode: process_command_mode(&mode),
        command: command_text.to_string(),
        args,
        executed_program: program.clone(),
        executed_args: command_args.clone(),
        environment_keys,
    })?;
    let mut command = Command::new(&program);
    command
        .args(&command_args)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !env.is_empty() {
        command.envs(env);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = format!("run process: {error}");
            kernel.fail_process_session(&process.process_id, message.clone())?;
            return Err(AgentOsError::Validation(message));
        }
    };
    kernel.mark_process_session_running(
        &process.process_id,
        Some(child.id()),
        Some(stdout_spool_path.to_string_lossy().into_owned()),
        Some(stderr_spool_path.to_string_lossy().into_owned()),
    )?;
    kernel.set_tool_worker_output_spool(
        tool_call_id,
        stdout_spool_path.to_string_lossy().into_owned(),
        stderr_spool_path.to_string_lossy().into_owned(),
    );
    let stdout_capture = Arc::new(Mutex::new(ToolStreamOutput {
        spool_path: Some(stdout_spool_path.to_string_lossy().into_owned()),
        ..ToolStreamOutput::default()
    }));
    let stderr_capture = Arc::new(Mutex::new(ToolStreamOutput {
        spool_path: Some(stderr_spool_path.to_string_lossy().into_owned()),
        ..ToolStreamOutput::default()
    }));
    let stdout_reader = child.stdout.take().map(|stdout| {
        spawn_stdout_reader(
            kernel.clone(),
            tool_call_id.to_string(),
            process.process_id.clone(),
            stdout,
            open_spool_file(&stdout_spool_path),
            stdout_capture.clone(),
        )
    });
    let stderr_reader = child.stderr.take().map(|stderr| {
        spawn_stderr_reader(
            kernel.clone(),
            tool_call_id.to_string(),
            process.process_id.clone(),
            stderr,
            open_spool_file(&stderr_spool_path),
            stderr_capture.clone(),
        )
    });
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            let message = format!("wait process: {error}");
            kernel.fail_process_session(&process.process_id, message.clone())?;
            return Err(AgentOsError::Validation(message));
        }
    };
    if let Err(error) = join_reader(stdout_reader) {
        kernel.fail_process_session(&process.process_id, error.to_string())?;
        return Err(error);
    }
    if let Err(error) = join_reader(stderr_reader) {
        kernel.fail_process_session(&process.process_id, error.to_string())?;
        return Err(error);
    }
    let stdout = stdout_capture
        .lock()
        .map_err(|_| AgentOsError::Validation("stdout capture lock poisoned".to_string()))?
        .clone();
    let stderr = stderr_capture
        .lock()
        .map_err(|_| AgentOsError::Validation("stderr capture lock poisoned".to_string()))?
        .clone();
    kernel.exit_process_session(
        &process.process_id,
        status.code(),
        process_output_stream(ProcessOutputStreamName::Stdout, &stdout),
        process_output_stream(ProcessOutputStreamName::Stderr, &stderr),
    )?;
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "process_id": process.process_id,
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "exit_code": status.code().unwrap_or(-1),
        "execution_mode": mode,
        "executed_program": program,
        "executed_args": command_args,
        "stdout": stdout.tail_window(super::super::builtin::run_command::OUTPUT_PREVIEW_CHARS).text,
        "stderr": stderr.tail_window(super::super::builtin::run_command::OUTPUT_PREVIEW_CHARS).text,
        "stdout_truncated": stdout.truncated,
        "stderr_truncated": stderr.truncated,
        "stdout_bytes": stdout.bytes,
        "stderr_bytes": stderr.bytes,
    }))
}

fn process_command_mode(mode: &str) -> ProcessCommandMode {
    match mode {
        "exec" => ProcessCommandMode::Exec,
        _ => ProcessCommandMode::Shell,
    }
}

fn process_output_stream(
    name: ProcessOutputStreamName,
    stream: &ToolStreamOutput,
) -> ProcessOutputStream {
    let bytes = stream.bytes as u64;
    ProcessOutputStream {
        name,
        sequence: 0,
        bytes,
        cursor: bytes,
        truncated: stream.truncated,
        spool_path: stream.spool_path.clone(),
    }
}

fn run_command_mode(input: &Value, has_args: bool) -> AgentOsResult<String> {
    let mode = optional_string(input, "mode")?.unwrap_or_else(|| {
        if has_args {
            "exec".to_string()
        } else {
            "shell".to_string()
        }
    });
    match mode.as_str() {
        "shell" | "exec" => Ok(mode),
        _ => Err(AgentOsError::Validation(
            "run_command mode must be shell or exec".to_string(),
        )),
    }
}

fn run_command_program_and_args(
    mode: &str,
    command_text: &str,
    args: Vec<String>,
) -> (String, Vec<String>) {
    if mode == "exec" {
        return (command_text.to_string(), args);
    }
    if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                command_text.to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-lc".to_string(), command_text.to_string()],
        )
    }
}

fn tool_output_spool_paths(tool_call_id: &str) -> AgentOsResult<(PathBuf, PathBuf)> {
    let directory = std::env::temp_dir().join("agent-os-tool-output");
    fs::create_dir_all(&directory)
        .map_err(|error| AgentOsError::Validation(format!("create tool output spool: {error}")))?;
    Ok((
        directory.join(format!("{tool_call_id}.stdout.log")),
        directory.join(format!("{tool_call_id}.stderr.log")),
    ))
}

fn open_spool_file(path: &Path) -> AgentOsResult<File> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|error| AgentOsError::Validation(format!("open tool output spool: {error}")))
}

fn spawn_stdout_reader(
    kernel: Kernel,
    tool_call_id: String,
    process_id: String,
    stdout: ChildStdout,
    spool: AgentOsResult<File>,
    capture: Arc<Mutex<ToolStreamOutput>>,
) -> JoinHandle<AgentOsResult<()>> {
    std::thread::spawn(move || {
        read_process_stream(
            kernel,
            tool_call_id,
            process_id,
            super::super::ToolOutputStream::Stdout,
            stdout,
            spool?,
            capture,
        )
    })
}

fn spawn_stderr_reader(
    kernel: Kernel,
    tool_call_id: String,
    process_id: String,
    stderr: ChildStderr,
    spool: AgentOsResult<File>,
    capture: Arc<Mutex<ToolStreamOutput>>,
) -> JoinHandle<AgentOsResult<()>> {
    std::thread::spawn(move || {
        read_process_stream(
            kernel,
            tool_call_id,
            process_id,
            super::super::ToolOutputStream::Stderr,
            stderr,
            spool?,
            capture,
        )
    })
}

fn read_process_stream<R: Read>(
    kernel: Kernel,
    tool_call_id: String,
    process_id: String,
    stream: super::super::ToolOutputStream,
    mut reader: R,
    mut spool: File,
    capture: Arc<Mutex<ToolStreamOutput>>,
) -> AgentOsResult<()> {
    let mut buffer = [0_u8; 4096];
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|error| AgentOsError::Validation(format!("read process output: {error}")))?;
        if bytes_read == 0 {
            return Ok(());
        }
        let chunk = &buffer[..bytes_read];
        spool.write_all(chunk).map_err(|error| {
            AgentOsError::Validation(format!("write process output spool: {error}"))
        })?;
        spool.flush().map_err(|error| {
            AgentOsError::Validation(format!("flush process output spool: {error}"))
        })?;
        {
            let mut capture = capture.lock().map_err(|_| {
                AgentOsError::Validation("process stream capture lock poisoned".to_string())
            })?;
            capture.append_bounded(chunk);
        }
        kernel.append_tool_worker_output(&tool_call_id, stream, chunk);
        kernel.append_process_output_chunk(
            &process_id,
            process_output_stream_name(stream),
            chunk,
        )?;
    }
}

fn process_output_stream_name(stream: super::super::ToolOutputStream) -> ProcessOutputStreamName {
    match stream {
        super::super::ToolOutputStream::Stdout => ProcessOutputStreamName::Stdout,
        super::super::ToolOutputStream::Stderr => ProcessOutputStreamName::Stderr,
    }
}

fn join_reader(reader: Option<JoinHandle<AgentOsResult<()>>>) -> AgentOsResult<()> {
    if let Some(reader) = reader {
        reader.join().map_err(|_| {
            AgentOsError::Validation("process output reader panicked".to_string())
        })??;
    }
    Ok(())
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

fn optional_string(input: &Value, field: &str) -> AgentOsResult<Option<String>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be a string")))
}

fn optional_string_array(input: &Value, field: &str) -> AgentOsResult<Option<Vec<String>>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_array()
        .ok_or_else(|| AgentOsError::Validation(format!("run_command {field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                AgentOsError::Validation(format!("run_command {field} values must be strings"))
            })
        })
        .collect::<AgentOsResult<Vec<_>>>()
        .map(Some)
}

fn image_mime_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "avif" => Some("image/avif"),
        "ico" => Some("image/x-icon"),
        _ => None,
    }
}

enum WorkspacePatch {
    Add {
        path: PathBuf,
        content: String,
    },
    Update {
        path: PathBuf,
        hunks: Vec<PatchHunk>,
    },
    Delete {
        path: PathBuf,
    },
}

impl WorkspacePatch {
    fn path(&self) -> &Path {
        match self {
            Self::Add { path, .. } | Self::Update { path, .. } | Self::Delete { path } => path,
        }
    }
}

struct PatchHunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    plain_old_lines: Vec<String>,
    plain_new_lines: Vec<String>,
    prefer_plain_context: bool,
}

const MULTIPLE_OPERATIONS_ERROR: &str = "apply_patch accepts exactly one file operation";

fn parse_apply_patch(patch: &str) -> AgentOsResult<WorkspacePatch> {
    let normalized = patch.replace("\r\n", "\n");
    let lines = normalized.split('\n').collect::<Vec<_>>();
    if lines.first() != Some(&"*** Begin Patch") {
        return Err(AgentOsError::Validation(
            "apply_patch must start with *** Begin Patch".to_string(),
        ));
    }
    let mut index = 1;
    while lines.get(index).is_some_and(|line| line.trim().is_empty()) {
        index += 1;
    }
    let Some(header) = lines.get(index) else {
        return Err(AgentOsError::Validation(
            "apply_patch missing file operation".to_string(),
        ));
    };
    index += 1;
    let operation = if let Some(path) = header.strip_prefix("*** Add File: ") {
        parse_add_file(path, &lines, &mut index)?
    } else if let Some(path) = header.strip_prefix("*** Update File: ") {
        parse_update_file(path, &lines, &mut index)?
    } else if let Some(path) = header.strip_prefix("*** Delete File: ") {
        WorkspacePatch::Delete {
            path: PathBuf::from(path),
        }
    } else {
        return Err(AgentOsError::Validation(
            "apply_patch supports Add File, Update File, or Delete File".to_string(),
        ));
    };
    require_patch_end(&lines, index)?;
    Ok(operation)
}

fn parse_add_file(path: &str, lines: &[&str], index: &mut usize) -> AgentOsResult<WorkspacePatch> {
    let mut content = Vec::new();
    while let Some(line) = lines.get(*index) {
        if *line == "*** End Patch" {
            break;
        }
        if line.starts_with("*** ") {
            return Err(AgentOsError::Validation(
                MULTIPLE_OPERATIONS_ERROR.to_string(),
            ));
        }
        let Some(text) = line.strip_prefix('+') else {
            return Err(AgentOsError::Validation(
                "apply_patch add file lines must start with +".to_string(),
            ));
        };
        content.push(text.to_string());
        *index += 1;
    }
    Ok(WorkspacePatch::Add {
        path: PathBuf::from(path),
        content: finish_patch_content(content),
    })
}

fn parse_update_file(
    path: &str,
    lines: &[&str],
    index: &mut usize,
) -> AgentOsResult<WorkspacePatch> {
    let mut hunks = Vec::new();
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut plain_old_lines = Vec::new();
    let mut plain_new_lines = Vec::new();
    let mut changed = false;
    let mut prefer_plain_context = false;
    while let Some(line) = lines.get(*index) {
        if *line == "*** End Patch" || *line == "*** End of File" {
            break;
        }
        if line.starts_with("*** ") {
            return Err(AgentOsError::Validation(
                MULTIPLE_OPERATIONS_ERROR.to_string(),
            ));
        }
        if line.starts_with("@@") {
            push_hunk(
                &mut hunks,
                &mut old_lines,
                &mut new_lines,
                &mut plain_old_lines,
                &mut plain_new_lines,
                changed,
                prefer_plain_context,
            )?;
            changed = false;
            prefer_plain_context = false;
            *index += 1;
            continue;
        }
        if line.is_empty() {
            old_lines.push(String::new());
            new_lines.push(String::new());
            plain_old_lines.push(String::new());
            plain_new_lines.push(String::new());
            *index += 1;
            continue;
        }
        let Some((prefix, text)) = line.split_at_checked(1) else {
            unreachable!("empty patch hunk lines are handled before splitting")
        };
        match prefix {
            " " => {
                old_lines.push(text.to_string());
                new_lines.push(text.to_string());
                plain_old_lines.push((*line).to_string());
                plain_new_lines.push((*line).to_string());
            }
            "-" => {
                old_lines.push(text.to_string());
                plain_old_lines.push(text.to_string());
                changed = true;
            }
            "+" => {
                new_lines.push(text.to_string());
                plain_new_lines.push(text.to_string());
                changed = true;
            }
            _ => {
                old_lines.push((*line).to_string());
                new_lines.push((*line).to_string());
                plain_old_lines.push((*line).to_string());
                plain_new_lines.push((*line).to_string());
                prefer_plain_context = true;
            }
        }
        *index += 1;
    }
    push_hunk(
        &mut hunks,
        &mut old_lines,
        &mut new_lines,
        &mut plain_old_lines,
        &mut plain_new_lines,
        changed,
        prefer_plain_context,
    )?;
    if hunks.is_empty() {
        return Err(AgentOsError::Validation(
            "apply_patch update operation must contain a changed hunk".to_string(),
        ));
    }
    Ok(WorkspacePatch::Update {
        path: PathBuf::from(path),
        hunks,
    })
}

fn push_hunk(
    hunks: &mut Vec<PatchHunk>,
    old_lines: &mut Vec<String>,
    new_lines: &mut Vec<String>,
    plain_old_lines: &mut Vec<String>,
    plain_new_lines: &mut Vec<String>,
    changed: bool,
    prefer_plain_context: bool,
) -> AgentOsResult<()> {
    if old_lines.is_empty() && new_lines.is_empty() {
        return Ok(());
    }
    if !changed {
        return Err(AgentOsError::Validation(
            "apply_patch update hunk must change at least one line".to_string(),
        ));
    }
    hunks.push(PatchHunk {
        old_lines: std::mem::take(old_lines),
        new_lines: std::mem::take(new_lines),
        plain_old_lines: std::mem::take(plain_old_lines),
        plain_new_lines: std::mem::take(plain_new_lines),
        prefer_plain_context,
    });
    Ok(())
}

fn require_patch_end(lines: &[&str], index: usize) -> AgentOsResult<()> {
    match lines.get(index) {
        Some(&"*** End Patch") => {}
        Some(line) if line.starts_with("*** ") => {
            return Err(AgentOsError::Validation(
                MULTIPLE_OPERATIONS_ERROR.to_string(),
            ))
        }
        _ => {
            return Err(AgentOsError::Validation(
                "apply_patch must end with *** End Patch".to_string(),
            ))
        }
    }
    if lines.iter().skip(index + 1).any(|line| !line.is_empty()) {
        return Err(AgentOsError::Validation(
            "apply_patch cannot contain content after *** End Patch".to_string(),
        ));
    }
    Ok(())
}

fn finish_patch_content(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn apply_update_hunks(before: &str, hunks: &[PatchHunk]) -> AgentOsResult<String> {
    let had_trailing_newline = before.ends_with('\n');
    let mut lines = before.lines().map(str::to_string).collect::<Vec<_>>();
    for hunk in hunks {
        let Some((position, old_len, new_lines)) = find_hunk_replacement(&lines, hunk) else {
            return Err(AgentOsError::Validation(
                "apply_patch update hunk did not match file content".to_string(),
            ));
        };
        lines.splice(position..position + old_len, new_lines);
    }
    let mut after = lines.join("\n");
    if had_trailing_newline && !after.is_empty() {
        after.push('\n');
    }
    Ok(after)
}

fn find_hunk_replacement(
    lines: &[String],
    hunk: &PatchHunk,
) -> Option<(usize, usize, Vec<String>)> {
    let canonical = find_hunk(lines, &hunk.old_lines)
        .map(|position| (position, hunk.old_lines.len(), hunk.new_lines.clone()));
    let plain_differs =
        hunk.plain_old_lines != hunk.old_lines || hunk.plain_new_lines != hunk.new_lines;
    let plain = if plain_differs {
        find_hunk(lines, &hunk.plain_old_lines).map(|position| {
            (
                position,
                hunk.plain_old_lines.len(),
                hunk.plain_new_lines.clone(),
            )
        })
    } else {
        None
    };

    if hunk.prefer_plain_context {
        plain.or(canonical)
    } else {
        canonical.or(plain)
    }
}

fn find_hunk(lines: &[String], old_lines: &[String]) -> Option<usize> {
    if old_lines.is_empty() || old_lines.len() > lines.len() {
        return None;
    }
    lines
        .windows(old_lines.len())
        .position(|candidate| candidate == old_lines)
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
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(AgentOsError::Validation(format!(
            "canonicalize workspace root: {error}"
        ))),
    }
}

fn ensure_workspace_target_contained(root: &Path, target: &Path) -> AgentOsResult<()> {
    if target.exists() {
        let canonical = target.canonicalize().map_err(|error| {
            AgentOsError::Validation(format!("canonicalize workspace path: {error}"))
        })?;
        if !canonical.starts_with(root) {
            return Err(AgentOsError::PermissionDenied(
                "workspace path escapes workspace root".to_string(),
            ));
        }
        return Ok(());
    }
    let mut ancestor = target.parent().ok_or_else(|| {
        AgentOsError::Validation("workspace path must have a parent directory".to_string())
    })?;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            AgentOsError::Validation("workspace path has no existing ancestor".to_string())
        })?;
    }
    let canonical_parent = ancestor
        .canonicalize()
        .map_err(|error| AgentOsError::Validation(format!("canonicalize parent path: {error}")))?;
    if !canonical_parent.starts_with(root) {
        return Err(AgentOsError::PermissionDenied(
            "workspace path escapes workspace root".to_string(),
        ));
    }
    Ok(())
}

fn preview_text(content: &str) -> String {
    content.chars().take(PREVIEW_CHARS).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_hunk_accepts_plain_context_lines() {
        let patch = "*** Begin Patch\n*** Update File: src/lib.rs\n@@\nfn demo() {\n    before();\n\n+    inserted();\n    after();\n}\n*** End Patch\n";
        let operation = parse_apply_patch(patch).unwrap();
        let WorkspacePatch::Update { hunks, .. } = operation else {
            panic!("expected update operation");
        };

        let before = "fn demo() {\n    before();\n\n    after();\n}\n";
        let after = apply_update_hunks(before, &hunks).unwrap();

        assert_eq!(
            after,
            "fn demo() {\n    before();\n\n    inserted();\n    after();\n}\n"
        );
    }

    #[test]
    fn update_hunk_still_rejects_plain_context_without_changes() {
        let patch = "*** Begin Patch\n*** Update File: src/lib.rs\n@@\nfn demo() {\n    before();\n}\n*** End Patch\n";
        let error = match parse_apply_patch(patch) {
            Ok(_) => panic!("expected no-op update hunk to fail"),
            Err(error) => error.to_string(),
        };

        assert!(
            error.contains("apply_patch update hunk must change at least one line"),
            "{error}"
        );
    }

    #[test]
    fn image_mime_type_maps_supported_extensions_and_rejects_svg() {
        assert_eq!(image_mime_type(Path::new("shot.PNG")), Some("image/png"));
        assert_eq!(image_mime_type(Path::new("photo.jpeg")), Some("image/jpeg"));
        assert_eq!(image_mime_type(Path::new("icon.svg")), None);
        assert_eq!(image_mime_type(Path::new("archive.zip")), None);
    }
}
