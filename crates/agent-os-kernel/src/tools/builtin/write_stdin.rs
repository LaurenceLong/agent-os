use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "write_stdin",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_write_stdin",
            name: "write_stdin",
            description: "Write stdin to a running process started by this agent, or poll that process output by process_id. Use after run_command returns a process_id with stdin_mode piped.",
            driver_class: ToolDriverClass::Shell,
            risk_level: 4,
            input_schema: schema::object(
                &["process_id"],
                json!({
                    "process_id": {"type": "string"},
                    "write_id": {"type": "string"},
                    "text": {"type": "string"},
                    "field": {"enum": ["stdout", "stderr"]},
                    "after_sequence": {
                        "type": "object",
                        "properties": {
                            "stdout": {"type": "integer", "minimum": 0},
                            "stderr": {"type": "integer", "minimum": 0}
                        },
                        "additionalProperties": false
                    }
                }),
            ),
            model_input_schema: schema::object(
                &["process_id"],
                json!({
                    "process_id": {
                        "type": "string",
                        "description": "Process id returned by this agent's run_command call."
                    },
                    "write_id": {
                        "type": "string",
                        "description": "Required with text. Stable id for idempotent retry of the stdin write."
                    },
                    "text": {
                        "type": "string",
                        "description": "Text bytes to write to piped stdin. Omit text to poll process output only."
                    },
                    "field": {
                        "enum": ["stdout", "stderr"],
                        "description": "Optional output stream filter for polling."
                    },
                    "after_sequence": {
                        "type": "object",
                        "description": "Optional per-stream process output sequence cursor for polling only new chunks.",
                        "properties": {
                            "stdout": {"type": "integer", "minimum": 0},
                            "stderr": {"type": "integer", "minimum": 0}
                        },
                        "additionalProperties": false
                    }
                }),
            ),
            examples: vec![
                schema::example(
                    "Write one line to a piped process stdin.",
                    json!({"process_id": "proc_example", "write_id": "stdin_1", "text": "continue\n"}),
                    "Writes the stdin text once for write_id and returns the process output window.",
                ),
                schema::example(
                    "Poll process stdout after a prior sequence.",
                    json!({"process_id": "proc_example", "field": "stdout", "after_sequence": {"stdout": 2}}),
                    "Returns process output chunks after the requested stdout sequence.",
                ),
            ],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "process_id",
                    "input",
                    "driver_class",
                    "tool_call_id",
                    "invocation",
                    "background_worker",
                    "fields",
                    "process_session",
                    "process_output",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "process_id": {"type": "string"},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "tool_call_id": {"type": "string"},
                    "invocation": {"type": "object"},
                    "background_worker": {"type": ["object", "null"]},
                    "fields": {"type": "object"},
                    "stdin_write": {"type": "object"},
                    "process_session": {"type": "object"},
                    "process_output": {"type": "object"}
                }),
            ),
            runtime_input_policy: ToolRuntimeInputPolicy::default(),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::CommandLog),
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
    super::super::driver::workspace::run_process_stdin(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_process_id_and_documents_idempotent_write() {
        let descriptor = descriptor("now");
        let required = descriptor
            .model_input_schema
            .as_ref()
            .unwrap()
            .pointer("/required")
            .and_then(Value::as_array)
            .unwrap();
        assert!(required.iter().any(|value| value == "process_id"));
        assert!(!required.iter().any(|value| value == "write_id"));
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters["write_id"] == "stdin_1"
                && example.expected_result.contains("once for write_id")));
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters["after_sequence"]["stdout"] == 2));
    }
}
