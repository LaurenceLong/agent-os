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
        name: "search_files",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_search_files",
            name: "search_files",
            description: "Search workspace file paths and text content without shelling out. Use it before read_file when the relevant files are unknown.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: schema::object(
            &["workspace_root", "query"],
            json!({
                "workspace_root": {"type": "string"},
                "query": {"type": "string"},
                "path": {"type": "string"},
                "mode": {"enum": ["path", "content", "both"]},
                "case_sensitive": {"type": "boolean"},
                "offset": {"type": "integer", "minimum": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT}
            }),
        ),
            model_input_schema: schema::object(
            &["query"],
            json!({
                "query": {"type": "string", "description": "Literal substring to find in workspace-relative paths and/or UTF-8 text file lines."},
                "path": {"type": "string", "description": "Optional workspace-relative file or directory scope. Defaults to the workspace root. Do not use absolute paths or '..'."},
                "mode": {"enum": ["path", "content", "both"], "description": "Defaults to both. Path mode matches workspace-relative paths; content mode matches UTF-8 file lines."},
                "case_sensitive": {"type": "boolean", "description": "Defaults to false."},
                "offset": {"type": "integer", "minimum": 0, "description": "Zero-based result offset. Defaults to 0."},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Maximum results to return. Defaults to 50 and is capped at 200."}
            }),
        ),
            examples: vec![
                schema::example(
                    "Find files or lines mentioning a Rust symbol.",
                    json!({"query": "ToolDescriptor", "path": "crates", "mode": "both", "limit": 50}),
                    "Returns bounded path/content matches with workspace-relative paths and line numbers for content matches.",
                ),
                schema::example(
                    "Find candidate instruction files by path.",
                    json!({"query": "AGENTS.md", "mode": "path"}),
                    "Returns matching workspace-relative paths without running a shell command.",
                ),
            ],
            output_schema: schema::object(
            &[
                "tool",
                "status",
                "input",
                "driver_class",
                "query",
                "mode",
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
                "query": {"type": "string"},
                "mode": {"enum": ["path", "content", "both"]},
                "path": {"type": "string"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer"},
                "total_matches": {"type": "integer"},
                "returned_matches": {"type": "integer"},
                "next_offset": {"type": ["integer", "null"]},
                "matches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["path", "match_kind"],
                        "properties": {
                            "path": {"type": "string"},
                            "match_kind": {"enum": ["path", "content"]},
                            "line_number": {"type": ["integer", "null"]},
                            "line": {"type": ["string", "null"]}
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
    super::super::driver::workspace::run_workspace_search_files(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_search_contract_and_hides_workspace_root() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();

        assert!(model_schema.pointer("/properties/query").is_some());
        assert!(model_schema.pointer("/properties/path").is_some());
        assert!(model_schema.pointer("/properties/workspace_root").is_none());
        assert_eq!(
            descriptor
                .runtime_input_policy
                .injected_fields
                .get("workspace_root")
                .map(String::as_str),
            Some("workspace_root")
        );
        assert!(descriptor.examples.iter().any(|example| {
            example.parameters == json!({"query": "AGENTS.md", "mode": "path"})
        }));
    }
}
