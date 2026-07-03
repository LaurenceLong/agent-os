use crate::state::{ToolStreamOutput, ToolWorkerRecord};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Map, Value};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;

const DEFAULT_NEW_LINES: usize = TOOL_OUTPUT_DEFAULT_NEW_LINES as usize;
const DEFAULT_PAGE_LINES: usize = TOOL_OUTPUT_DEFAULT_PAGE_LINES as usize;
const MAX_LINES: usize = TOOL_OUTPUT_MAX_LINES as usize;

pub(super) fn attach_output_management(
    kernel: &Kernel,
    call_id: &str,
    mut output: Value,
) -> AgentOsResult<Value> {
    let Some(object) = output.as_object_mut() else {
        return Ok(output);
    };
    let worker = kernel
        .tool_workers
        .lock()
        .ok()
        .and_then(|workers| workers.get(call_id).cloned());
    let mut fields = Map::new();
    if let Some(worker) = worker.as_ref() {
        insert_worker_stream(&mut fields, "stdout", &worker.output.stdout);
        insert_worker_stream(&mut fields, "stderr", &worker.output.stderr);
    }
    for (field, value) in object.iter() {
        if fields.contains_key(field) || !is_managed_field(field, value) {
            continue;
        }
        let Some(text) = value.as_str() else {
            continue;
        };
        let stream = spool_text_field(call_id, field, text)?;
        insert_stream(&mut fields, field, &stream);
    }
    if fields.is_empty() {
        return Ok(output);
    }
    object.insert(
        "_tool_output".to_string(),
        json!({
            "version": 1,
            "default_mode": "new",
            "default_new_lines": DEFAULT_NEW_LINES,
            "default_page_lines": DEFAULT_PAGE_LINES,
            "max_lines": MAX_LINES,
            "max_window_bytes": TOOL_OUTPUT_MAX_WINDOW_BYTES,
            "fields": fields
        }),
    );
    Ok(output)
}

pub(super) fn query_tool_output(
    invocation: &ToolInvocation,
    worker: Option<&ToolWorkerRecord>,
    payload: &Value,
) -> AgentOsResult<Value> {
    let field_filter = payload.get("field").and_then(Value::as_str);
    let fields = managed_fields(invocation, worker, field_filter)?;
    let mut result_fields = Map::new();
    for (field, stream) in fields {
        result_fields.insert(field.clone(), query_stream(&field, &stream, payload)?);
    }
    Ok(json!({
        "tool_call_id": invocation.call_id,
        "invocation": invocation,
        "background_worker": worker.map(worker_json),
        "fields": result_fields,
    }))
}

fn managed_fields(
    invocation: &ToolInvocation,
    worker: Option<&ToolWorkerRecord>,
    field_filter: Option<&str>,
) -> AgentOsResult<Vec<(String, ToolStreamOutput)>> {
    let mut fields = Vec::new();
    if let Some(worker) = worker {
        push_if_matches(
            &mut fields,
            "stdout",
            worker.output.stdout.clone(),
            field_filter,
        );
        push_if_matches(
            &mut fields,
            "stderr",
            worker.output.stderr.clone(),
            field_filter,
        );
    }
    if let Some(output) = invocation.output.as_ref() {
        if let Some(managed) = output
            .pointer("/_tool_output/fields")
            .and_then(Value::as_object)
        {
            for (field, metadata) in managed {
                if fields.iter().any(|(existing, _)| existing == field) {
                    continue;
                }
                if field_filter.is_some_and(|wanted| wanted != field) {
                    continue;
                }
                fields.push((field.clone(), stream_from_metadata(metadata)?));
            }
        }
    }
    Ok(fields)
}

fn push_if_matches(
    fields: &mut Vec<(String, ToolStreamOutput)>,
    field: &str,
    stream: ToolStreamOutput,
    field_filter: Option<&str>,
) {
    if field_filter.is_none_or(|wanted| wanted == field) {
        fields.push((field.to_string(), stream));
    }
}

fn query_stream(field: &str, stream: &ToolStreamOutput, payload: &Value) -> AgentOsResult<Value> {
    if payload
        .get("full")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || payload.get("offset").is_some()
    {
        return query_stream_page(field, stream, payload);
    }
    let head = payload
        .get("head")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let tail = payload
        .get("tail")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let new = payload
        .get("new")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| (head.is_none() && tail.is_none()).then_some(DEFAULT_NEW_LINES));
    let cursor = cursor_for_stream(payload, field);
    let mut result = Map::new();
    result.insert("bytes".to_string(), json!(stream.bytes));
    result.insert("truncated".to_string(), json!(stream.truncated));
    result.insert("next_cursor".to_string(), json!(stream.bytes));
    if let Some(head) = head {
        result.insert(
            "head".to_string(),
            window_json(first_lines(
                stream.head_window(max_window_bytes()),
                head.min(MAX_LINES),
            )),
        );
    }
    if let Some(tail) = tail {
        result.insert(
            "tail".to_string(),
            window_json(last_lines(
                stream.tail_window(max_window_bytes()),
                tail.min(MAX_LINES),
            )),
        );
    }
    if let Some(new) = new {
        result.insert(
            "new".to_string(),
            window_json(first_lines(
                stream.new_window(cursor, max_window_bytes()),
                new.min(MAX_LINES),
            )),
        );
    }
    Ok(Value::Object(result))
}

fn query_stream_page(
    field: &str,
    stream: &ToolStreamOutput,
    payload: &Value,
) -> AgentOsResult<Value> {
    let offset = payload
        .get("offset")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_PAGE_LINES)
        .clamp(1, MAX_LINES);
    let page = read_spooled_page(stream, offset, limit)?;
    Ok(json!({
        "field": field,
        "mode": "page",
        "offset": offset,
        "limit": limit,
        "content": page.content,
        "total_lines": page.total_lines,
        "returned_lines": page.returned_lines,
        "next_offset": page.next_offset,
        "truncated": page.truncated,
        "omitted_lines": page.omitted_lines,
        "bytes": stream.bytes,
    }))
}

struct OutputPage {
    content: String,
    total_lines: usize,
    returned_lines: usize,
    next_offset: Option<usize>,
    truncated: bool,
    omitted_lines: usize,
}

fn read_spooled_page(
    stream: &ToolStreamOutput,
    offset: usize,
    limit: usize,
) -> AgentOsResult<OutputPage> {
    let Some(path) = stream.spool_path.as_ref() else {
        let text = String::from_utf8_lossy(&stream.tail).to_string();
        return Ok(page_text(&text, offset, limit));
    };
    let file = File::open(path)
        .map_err(|error| AgentOsError::Validation(format!("open tool output spool: {error}")))?;
    let reader = BufReader::new(file);
    let mut total_lines = 0;
    let mut returned_lines = 0;
    let mut content = String::new();
    for line in reader.lines() {
        let line = line.map_err(|error| {
            AgentOsError::Validation(format!("read tool output spool: {error}"))
        })?;
        if total_lines >= offset && returned_lines < limit {
            content.push_str(&line);
            content.push('\n');
            returned_lines += 1;
        }
        total_lines += 1;
    }
    let next_offset = (offset + returned_lines < total_lines).then_some(offset + returned_lines);
    Ok(OutputPage {
        content,
        total_lines,
        returned_lines,
        next_offset,
        truncated: next_offset.is_some(),
        omitted_lines: total_lines.saturating_sub(offset + returned_lines),
    })
}

fn page_text(text: &str, offset: usize, limit: usize) -> OutputPage {
    let lines = text.lines().collect::<Vec<_>>();
    let total_lines = lines.len();
    let selected = lines
        .iter()
        .skip(offset)
        .take(limit)
        .copied()
        .collect::<Vec<_>>();
    let returned_lines = selected.len();
    let content = if selected.is_empty() {
        String::new()
    } else {
        format!("{}\n", selected.join("\n"))
    };
    let next_offset = (offset + returned_lines < total_lines).then_some(offset + returned_lines);
    OutputPage {
        content,
        total_lines,
        returned_lines,
        next_offset,
        truncated: next_offset.is_some(),
        omitted_lines: total_lines.saturating_sub(offset + returned_lines),
    }
}

fn worker_json(worker: &ToolWorkerRecord) -> Value {
    json!({
        "call_id": worker.call_id,
        "tool_name": worker.tool_name,
        "started_at": worker.started_at,
        "updated_at": worker.output.updated_at,
    })
}

fn insert_worker_stream(fields: &mut Map<String, Value>, field: &str, stream: &ToolStreamOutput) {
    if stream.bytes > 0 || stream.spool_path.is_some() {
        insert_stream(fields, field, stream);
    }
}

fn insert_stream(fields: &mut Map<String, Value>, field: &str, stream: &ToolStreamOutput) {
    fields.insert(
        field.to_string(),
        json!({
            "kind": "text",
            "bytes": stream.bytes,
            "truncated": stream.truncated,
            "spool_path": stream.spool_path,
        }),
    );
}

fn stream_from_metadata(metadata: &Value) -> AgentOsResult<ToolStreamOutput> {
    let spool_path = metadata
        .get("spool_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    let bytes = metadata
        .get("bytes")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let truncated = metadata
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut stream = ToolStreamOutput {
        bytes,
        truncated,
        spool_path,
        ..ToolStreamOutput::default()
    };
    if let Some(path) = stream.spool_path.as_ref() {
        let mut file = File::open(path).map_err(|error| {
            AgentOsError::Validation(format!("open managed tool output field: {error}"))
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| {
            AgentOsError::Validation(format!("read managed tool output field: {error}"))
        })?;
        stream.head = bytes.iter().take(max_window_bytes()).copied().collect();
        stream.tail = bytes
            .iter()
            .skip(bytes.len().saturating_sub(max_window_bytes()))
            .copied()
            .collect();
    }
    Ok(stream)
}

fn spool_text_field(call_id: &str, field: &str, text: &str) -> AgentOsResult<ToolStreamOutput> {
    let path = spool_path(call_id, field)?;
    let mut file = File::create(&path).map_err(|error| {
        AgentOsError::Validation(format!("create tool output field spool: {error}"))
    })?;
    file.write_all(text.as_bytes()).map_err(|error| {
        AgentOsError::Validation(format!("write tool output field spool: {error}"))
    })?;
    let mut stream = ToolStreamOutput {
        spool_path: Some(path.to_string_lossy().into_owned()),
        ..ToolStreamOutput::default()
    };
    stream.append_bounded(text.as_bytes());
    Ok(stream)
}

fn spool_path(call_id: &str, field: &str) -> AgentOsResult<PathBuf> {
    let directory = std::env::temp_dir().join("agent-os-tool-output");
    fs::create_dir_all(&directory)
        .map_err(|error| AgentOsError::Validation(format!("create tool output spool: {error}")))?;
    Ok(directory.join(format!("{call_id}.{}.log", safe_field_name(field))))
}

fn safe_field_name(field: &str) -> String {
    field
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn is_managed_field(field: &str, value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    matches!(
        field,
        "content"
            | "stdout"
            | "stderr"
            | "preview"
            | "raw_result"
            | "message"
            | "question"
            | "summary"
    ) || text.len() > 512
}

fn cursor_for_stream(payload: &Value, stream_name: &str) -> usize {
    let Some(cursor) = payload.get("cursor") else {
        return 0;
    };
    if let Some(value) = cursor.as_u64() {
        return usize::try_from(value).unwrap_or(usize::MAX);
    }
    let Some(cursor) = cursor.as_object() else {
        return 0;
    };
    cursor
        .get(stream_name)
        .or_else(|| cursor.get(&format!("{stream_name}_bytes")))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn window_json(window: super::StreamWindow) -> Value {
    json!({
        "text": window.text,
        "start_byte": window.start_byte,
        "end_byte": window.end_byte,
        "truncated": window.truncated,
    })
}

fn max_window_bytes() -> usize {
    TOOL_OUTPUT_MAX_WINDOW_BYTES as usize
}

fn first_lines(window: super::StreamWindow, line_limit: usize) -> super::StreamWindow {
    if line_limit == 0 || window.text.is_empty() {
        return super::StreamWindow {
            text: String::new(),
            start_byte: window.start_byte,
            end_byte: window.start_byte,
            truncated: window.truncated || !window.text.is_empty(),
        };
    }
    let bytes = window.text.as_bytes();
    let selected_len = first_line_bytes(bytes, line_limit).min(max_window_bytes());
    super::StreamWindow {
        text: String::from_utf8_lossy(&bytes[..selected_len]).to_string(),
        start_byte: window.start_byte,
        end_byte: window.start_byte + selected_len,
        truncated: window.truncated || selected_len < bytes.len(),
    }
}

fn last_lines(window: super::StreamWindow, line_limit: usize) -> super::StreamWindow {
    if line_limit == 0 || window.text.is_empty() {
        return super::StreamWindow {
            text: String::new(),
            start_byte: window.end_byte,
            end_byte: window.end_byte,
            truncated: window.truncated || !window.text.is_empty(),
        };
    }
    let bytes = window.text.as_bytes();
    let start =
        last_line_start(bytes, line_limit).max(bytes.len().saturating_sub(max_window_bytes()));
    super::StreamWindow {
        text: String::from_utf8_lossy(&bytes[start..]).to_string(),
        start_byte: window.start_byte + start,
        end_byte: window.end_byte,
        truncated: window.truncated || start > 0,
    }
}

fn first_line_bytes(bytes: &[u8], line_limit: usize) -> usize {
    let mut lines = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines += 1;
            if lines == line_limit {
                return index + 1;
            }
        }
    }
    bytes.len()
}

fn last_line_start(bytes: &[u8], line_limit: usize) -> usize {
    let mut lines = 0;
    for index in (0..bytes.len()).rev() {
        if bytes[index] == b'\n' {
            lines += 1;
            if lines > line_limit {
                return index + 1;
            }
        }
    }
    0
}
