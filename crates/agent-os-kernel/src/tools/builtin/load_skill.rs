use super::{read_file, schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "load_skill",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_load_skill",
            name: "load_skill",
            description: "Load one imported skill's SKILL.md content page by page by skill name.",
            driver_class: ToolDriverClass::KernelBuiltin,
            risk_level: 1,
            input_schema: input_schema(),
            model_input_schema: schema::object(
                &["name"],
                json!({
                    "name": {"type": "string", "description": "Imported skill name."},
                    "offset": {"type": "integer", "minimum": 1, "description": "One-based starting line offset. Defaults to 1, which starts before the first line and includes line 1."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": read_file::MAX_LIMIT, "description": "Maximum lines to return. Defaults to 200 and is capped at 1000."}
                }),
            ),
            examples: vec![schema::example(
                "Load the first page of an imported skill.",
                json!({"name": "frontend-design", "offset": 1, "limit": 200}),
                "Returns the SKILL.md page with pagination metadata.",
            )],
            output_schema: skill_output_schema(&[
                "tool",
                "status",
                "input",
                "driver_class",
                "skill_id",
                "name",
                "description",
                "content",
                "offset",
                "limit",
                "total_lines",
                "returned_lines",
                "truncated",
                "omitted_lines",
            ]),
            runtime_input_policy: schema::required_scopes(&["skill:*"]),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::SourceRef),
        },
    )
}

fn input_schema() -> Value {
    schema::object(
        &["name"],
        json!({
            "name": {"type": "string"},
            "offset": {"type": "integer", "minimum": 1},
            "limit": {"type": "integer", "minimum": 1, "maximum": read_file::MAX_LIMIT}
        }),
    )
}

fn skill_output_schema(required: &[&str]) -> Value {
    schema::object(
        required,
        json!({
            "tool": {"type": "string"},
            "status": {"enum": ["ok"]},
            "input": {"type": "object"},
            "driver_class": {"type": "string"},
            "skill_id": {"type": "string"},
            "name": {"type": "string"},
            "description": {"type": "string"},
            "content": {"type": "string"},
            "root_path": {"type": "string"},
            "skill_file_path": {"type": "string"},
            "offset": {"type": "integer"},
            "limit": {"type": "integer"},
            "total_lines": {"type": "integer"},
            "returned_lines": {"type": "integer"},
            "next_offset": {"type": ["integer", "null"]},
            "truncated": {"type": "boolean"},
            "omitted_lines": {"type": "integer"}
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
    super::super::driver::ecosystem::run_load_skill(kernel, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_paging() {
        let descriptor = descriptor("now");
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
