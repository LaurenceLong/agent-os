use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) const DEFAULT_LIMIT: usize = 200;
pub(super) const MAX_LIMIT: usize = 1000;

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "read_file",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_read_file",
            name: "read_file",
            description:
                "Read a workspace file page by page. Use offset and limit to avoid loading large files into model context.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: schema::object(
            &["workspace_root", "path"],
            json!({
                "workspace_root": {"type": "string"},
                "path": {"type": "string"},
                "offset": {"type": "integer", "minimum": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT}
            }),
        ),
            model_input_schema: schema::object(
            &["path"],
            json!({
                "path": {"type": "string", "description": "Workspace-relative path to read. Do not use absolute paths or '..'."},
                "offset": {"type": "integer", "minimum": 0, "description": "Zero-based line offset. Defaults to 0."},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Maximum lines to return. Defaults to 200 and is capped at 1000."}
            }),
        ),
            examples: vec![schema::example(
                "Read the first bounded page of a workspace file.",
                json!({"path": "src/lib.rs", "offset": 0, "limit": 120}),
                "Returns file content plus total_lines, returned_lines, next_offset, and truncation metadata.",
            )],
            output_schema: schema::object(
            &[
                "tool",
                "status",
                "input",
                "driver_class",
                "path",
                "content",
                "bytes_read",
                "offset",
                "limit",
                "total_lines",
                "returned_lines",
                "truncated",
                "omitted_lines",
            ],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "path": {"type": "string"},
                "content": {"type": "string"},
                "bytes_read": {"type": "integer"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer"},
                "total_lines": {"type": "integer"},
                "returned_lines": {"type": "integer"},
                "next_offset": {"type": ["integer", "null"]},
                "truncated": {"type": "boolean"},
                "omitted_lines": {"type": "integer"}
            }),
        ),
            runtime_input_policy: schema::injected_workspace_root("workspace_root"),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
        },
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::workspace::run_workspace_read_file(kernel, syscall, descriptor, input)
}

pub(in crate::tools) fn parse_paging(input: &Value) -> AgentOsResult<(usize, usize)> {
    let offset = optional_usize(input, "offset")?.unwrap_or(0);
    let limit = optional_usize(input, "limit")?.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(AgentOsError::Validation(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    Ok((offset, limit))
}

pub(in crate::tools) struct PagedText {
    pub content: String,
    pub total_lines: usize,
    pub returned_lines: usize,
    pub next_offset: Option<usize>,
    pub truncated: bool,
    pub omitted_lines: usize,
}

pub(in crate::tools) fn paginate_text(content: &str, offset: usize, limit: usize) -> PagedText {
    let lines = if content.is_empty() {
        Vec::new()
    } else {
        content.split_inclusive('\n').collect::<Vec<_>>()
    };
    let total_lines = lines.len();
    let start = offset.min(total_lines);
    let end = start.saturating_add(limit).min(total_lines);
    let page = lines[start..end].concat();
    let returned_lines = end.saturating_sub(start);
    let truncated = end < total_lines;
    PagedText {
        content: page,
        total_lines,
        returned_lines,
        next_offset: truncated.then_some(end),
        truncated,
        omitted_lines: total_lines.saturating_sub(end),
    }
}

fn optional_usize(input: &Value, field: &str) -> AgentOsResult<Option<usize>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be a non-negative integer")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_offset_limit_and_hides_workspace_root_from_model() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();
        assert!(model_schema.pointer("/properties/offset").is_some());
        assert!(model_schema.pointer("/properties/limit").is_some());
        assert!(model_schema.pointer("/properties/workspace_root").is_none());
        assert_eq!(
            descriptor
                .runtime_input_policy
                .injected_fields
                .get("workspace_root")
                .map(String::as_str),
            Some("workspace_root")
        );
    }

    #[test]
    fn parse_paging_defaults_and_rejects_invalid_limit() {
        assert_eq!(parse_paging(&json!({})).unwrap(), (0, DEFAULT_LIMIT));
        assert_eq!(
            parse_paging(&json!({"offset": 10, "limit": 20})).unwrap(),
            (10, 20)
        );
        assert!(parse_paging(&json!({"limit": 0})).is_err());
        assert!(parse_paging(&json!({"limit": MAX_LIMIT + 1})).is_err());
        assert!(parse_paging(&json!({"offset": "1"})).is_err());
    }

    #[test]
    fn paginate_text_preserves_line_boundaries() {
        let page = paginate_text("a\nb\nc\n", 1, 1);
        assert_eq!(page.content, "b\n");
        assert_eq!(page.total_lines, 3);
        assert_eq!(page.returned_lines, 1);
        assert_eq!(page.next_offset, Some(2));
        assert!(page.truncated);
        assert_eq!(page.omitted_lines, 1);
    }
}
