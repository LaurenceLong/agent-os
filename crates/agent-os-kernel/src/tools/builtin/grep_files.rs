use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) const DEFAULT_LIMIT: usize = 50;
pub(in crate::tools) const MAX_LIMIT: usize = 200;
pub(in crate::tools) const MAX_RESULTS: usize = 1_000;
pub(in crate::tools) const MAX_FILE_BYTES: u64 = 1_000_000;
pub(in crate::tools) const MAX_VISITED_FILES: usize = 20_000;

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "grep_files",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_grep_files",
            name: "grep_files",
            description: "Search UTF-8 workspace file contents by literal text without shelling out. Use include to narrow candidate file paths by glob.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: schema::object(
            &["workspace_root", "pattern"],
            json!({
                "workspace_root": {"type": "string"},
                "pattern": {"type": "string"},
                "path": {"type": "string"},
                "include": {"type": "string"},
                "case_sensitive": {"type": "boolean"},
                "offset": {"type": "integer", "minimum": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT}
            }),
        ),
            model_input_schema: schema::object(
            &["pattern"],
            json!({
                "pattern": {"type": "string", "description": "Literal text to find in UTF-8 workspace file lines. This is not a regular expression."},
                "path": {"type": "string", "description": "Optional workspace-relative file or directory scope. Defaults to the workspace root. Do not use absolute paths or '..'."},
                "include": {"type": "string", "description": "Optional file glob relative to the search scope, such as \"**/*.rs\". Defaults to all files."},
                "case_sensitive": {"type": "boolean", "description": "Defaults to false."},
                "offset": {"type": "integer", "minimum": 0, "description": "Zero-based result offset. Defaults to 0."},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Maximum matches to return. Defaults to 50 and is capped at 200."}
            }),
        ),
            examples: vec![
                schema::example(
                    "Find lines mentioning a Rust symbol.",
                    json!({"pattern": "ToolDescriptor", "path": "crates", "include": "**/*.rs", "limit": 50}),
                    "Returns bounded content matches with workspace-relative paths, line numbers, and line previews.",
                ),
                schema::example(
                    "Page case-sensitive matches inside a directory scope.",
                    json!({"path": "notes", "include": "*.txt", "pattern": "Needle", "case_sensitive": true, "offset": 1, "limit": 1}),
                    "Searches only txt files under notes, keeps case-sensitive matches, skips the first match, and returns one result.",
                ),
                schema::example(
                    "Find instruction text anywhere in the workspace.",
                    json!({"pattern": "forward-only", "limit": 20}),
                    "Returns matching UTF-8 file lines without running a shell command.",
                ),
            ],
            output_schema: schema::object(
            &[
                "tool",
                "status",
                "input",
                "driver_class",
                "pattern",
                "path",
                "offset",
                "limit",
                "total_matches",
                "returned_matches",
                "matches",
                "truncated",
                "files_searched",
                "files_skipped",
            ],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "pattern": {"type": "string"},
                "path": {"type": "string"},
                "include": {"type": ["string", "null"]},
                "case_sensitive": {"type": "boolean"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer"},
                "total_matches": {"type": "integer"},
                "returned_matches": {"type": "integer"},
                "next_offset": {"type": ["integer", "null"]},
                "matches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["path", "line_number", "line"],
                        "properties": {
                            "path": {"type": "string"},
                            "line_number": {"type": "integer"},
                            "line": {"type": "string"}
                        },
                        "additionalProperties": false
                    }
                },
                "truncated": {"type": "boolean"},
                "files_searched": {"type": "integer"},
                "files_skipped": {"type": "integer"}
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
    super::super::driver::workspace::run_workspace_grep_files(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_grep_contract_and_hides_workspace_root() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();

        assert!(model_schema.pointer("/properties/pattern").is_some());
        assert!(model_schema.pointer("/properties/include").is_some());
        assert!(model_schema.pointer("/properties/workspace_root").is_none());
        assert_eq!(
            descriptor
                .runtime_input_policy
                .injected_fields
                .get("workspace_root")
                .map(String::as_str),
            Some("workspace_root")
        );
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters.get("include").is_some()));
        assert!(descriptor.examples.iter().any(|example| {
            example.parameters
                == json!({"path": "notes", "include": "*.txt", "pattern": "Needle", "case_sensitive": true, "offset": 1, "limit": 1})
        }));
    }
}
