use super::{read_file, schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "read_skill_resource",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_read_skill_resource",
            name: "read_skill_resource",
            description:
                "Read one imported skill resource page by page without allowing path escape.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: input_schema(),
            model_input_schema: schema::object(
                &["name", "path"],
                json!({
                    "name": {"type": "string", "description": "Imported skill name."},
                    "path": {"type": "string", "description": "Skill-root-relative resource path."},
                    "offset": {"type": "integer", "minimum": 1, "description": "One-based starting line offset. Defaults to 1, which starts before the first line and includes line 1."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": read_file::MAX_LIMIT, "description": "Maximum lines to return. Defaults to 200 and is capped at 1000."}
                }),
            ),
            examples: vec![schema::example(
                "Read a bounded page from a loaded skill resource.",
                json!({"name": "frontend-design", "path": "references/layout.md", "offset": 1, "limit": 120}),
                "Returns the resource page with pagination and path-boundary metadata.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "skill_id",
                    "name",
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
                    "skill_id": {"type": "string"},
                    "name": {"type": "string"},
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
            runtime_input_policy: schema::required_scopes(&["skill:*", "skill_file:*"]),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
        &["name", "path"],
        json!({
            "name": {"type": "string"},
            "path": {"type": "string"},
            "offset": {"type": "integer", "minimum": 1},
            "limit": {"type": "integer", "minimum": 1, "maximum": read_file::MAX_LIMIT}
        }),
    )
}

fn execute(
    kernel: &Kernel,
    _syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    _tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::ecosystem::run_read_skill_resource(kernel, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_name_and_path_and_exposes_paging() {
        let descriptor = descriptor("now");
        let required = descriptor.input_schema["required"]
            .as_array()
            .unwrap()
            .clone();
        assert!(required.iter().any(|value| value == "name"));
        assert!(required.iter().any(|value| value == "path"));
        assert!(descriptor
            .input_schema
            .pointer("/properties/offset")
            .is_some());
        assert!(descriptor
            .input_schema
            .pointer("/properties/limit")
            .is_some());
    }
}
