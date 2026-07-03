use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) const DEFAULT_LIMIT: usize = 50;
pub(in crate::tools) const MAX_LIMIT: usize = 200;
pub(in crate::tools) const MAX_RESULTS: usize = 1_000;
pub(in crate::tools) const MAX_VISITED_FILES: usize = 20_000;

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "glob_files",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_glob_files",
            name: "glob_files",
            description: "Find workspace file paths by glob pattern without shelling out. Use it before read_file when the relevant files are unknown and can be described by path shape.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: schema::object(
            &["workspace_root", "pattern"],
            json!({
                "workspace_root": {"type": "string"},
                "pattern": {"type": "string"},
                "path": {"type": "string"},
                "offset": {"type": "integer", "minimum": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT}
            }),
        ),
            model_input_schema: schema::object(
            &["pattern"],
            json!({
                "pattern": {"type": "string", "description": "Workspace file glob such as \"**/*.rs\" or \"crates/**/Cargo.toml\". Supports *, **, ?, and simple {a,b} alternates."},
                "path": {"type": "string", "description": "Optional workspace-relative directory scope. Defaults to the workspace root. Do not use absolute paths or '..'."},
                "offset": {"type": "integer", "minimum": 0, "description": "Zero-based result offset. Defaults to 0."},
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Maximum results to return. Defaults to 50 and is capped at 200."}
            }),
        ),
            examples: vec![
                schema::example(
                    "Find Rust files under crates.",
                    json!({"pattern": "**/*.rs", "path": "crates", "limit": 50}),
                    "Returns bounded workspace-relative file paths matching the glob.",
                ),
                schema::example(
                    "Find instruction files by path shape.",
                    json!({"pattern": "**/AGENTS.md"}),
                    "Returns matching workspace-relative file paths without running a shell command.",
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
                "offset": {"type": "integer"},
                "limit": {"type": "integer"},
                "total_matches": {"type": "integer"},
                "returned_matches": {"type": "integer"},
                "next_offset": {"type": ["integer", "null"]},
                "matches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["path"],
                        "properties": {
                            "path": {"type": "string"}
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
    super::super::driver::workspace::run_workspace_glob_files(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_glob_contract_and_hides_workspace_root() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();

        assert!(model_schema.pointer("/properties/pattern").is_some());
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
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters == json!({"pattern": "**/AGENTS.md"})));
    }
}
