use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) const OUTPUT_PREVIEW_CHARS: usize = 8_000;

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "run_command",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_run_command",
            name: "run_command",
            description: "Run a shell command in the workspace by default, or run one executable with explicit args in exec mode. Captures bounded stdout, stderr, exit code, and truncation metadata.",
            driver_class: ToolDriverClass::Shell,
            risk_level: 4,
            input_schema: schema::object(
            &["command", "cwd"],
            json!({
                "command": {"type": "string"},
                "mode": {"enum": ["shell", "exec"]},
                "args": {"type": "array", "items": {"type": "string"}},
                "cwd": {"type": "string"},
                "env": {
                    "type": "object",
                    "additionalProperties": {"type": "string"}
                }
            }),
        ),
            model_input_schema: schema::object(
            &["command"],
            json!({
                "command": {
                    "type": "string",
                    "description": "Shell command by default. In exec mode, this is the executable name or path."
                },
                "mode": {
                    "enum": ["shell", "exec"],
                    "description": "Defaults to shell. If args is present and mode is omitted, exec mode is inferred."
                },
                "args": {
                    "type": "array",
                    "description": "Optional exec-mode arguments for command. Omit for normal shell command strings.",
                    "items": {"type": "string"}
                },
                "env": {
                    "type": "object",
                    "description": "Optional per-command environment variables.",
                    "additionalProperties": {"type": "string"}
                }
            }),
        ),
            examples: vec![
                schema::example(
                    "Search the workspace with the default shell command mode.",
                    json!({"command": "rg -n \"TODO\" crates"}),
                    "Returns exit_code plus bounded stdout/stderr and truncation metadata.",
                ),
                schema::example(
                    "Run a focused Python test through shell syntax.",
                    json!({"command": "python -m pytest tests/test_api.py"}),
                    "Returns the test command exit code plus bounded stdout/stderr.",
                ),
                schema::example(
                    "Run one executable with explicit argv when shell parsing is not wanted.",
                    json!({"mode": "exec", "command": "cargo", "args": ["test", "-p", "agent-os-kernel"]}),
                    "Returns exit_code plus bounded stdout/stderr and truncation metadata.",
                ),
            ],
            output_schema: schema::object(
            &[
                "tool",
                "status",
                "input",
                "driver_class",
                "exit_code",
                "stdout",
                "stderr",
                "stdout_truncated",
                "stderr_truncated",
            ],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "exit_code": {"type": "integer"},
                "stdout": {"type": "string"},
                "stderr": {"type": "string"},
                "stdout_truncated": {"type": "boolean"},
                "stderr_truncated": {"type": "boolean"},
                "execution_mode": {"enum": ["shell", "exec"]},
                "executed_program": {"type": "string"},
                "executed_args": {"type": "array", "items": {"type": "string"}},
                "stdout_bytes": {"type": "integer"},
                "stderr_bytes": {"type": "integer"}
            }),
        ),
            runtime_input_policy: schema::injected_workspace_root("cwd"),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::CommandLog),
        },
    )
}

fn execute(
    kernel: &Kernel,
    syscall: &SyscallEnvelope,
    descriptor: &ToolDescriptor,
    tool_call_id: &str,
    input: &Value,
) -> AgentOsResult<Value> {
    super::super::driver::workspace::run_process(kernel, syscall, descriptor, tool_call_id, input)
}

pub(in crate::tools) fn bounded_text(text: &[u8]) -> (String, bool, usize) {
    let decoded = String::from_utf8_lossy(text);
    let total = decoded.len();
    if total <= OUTPUT_PREVIEW_CHARS {
        (decoded.to_string(), false, total)
    } else {
        (
            decoded.chars().take(OUTPUT_PREVIEW_CHARS).collect(),
            true,
            total,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_command_and_injects_cwd() {
        let descriptor = descriptor("now");
        let required = descriptor
            .model_input_schema
            .as_ref()
            .unwrap()
            .pointer("/required")
            .and_then(Value::as_array)
            .unwrap();
        assert!(required.iter().any(|value| value == "command"));
        assert!(!required.iter().any(|value| value == "args"));
        assert!(descriptor
            .model_input_schema
            .as_ref()
            .unwrap()
            .pointer("/properties/cwd")
            .is_none());
        assert_eq!(
            descriptor
                .runtime_input_policy
                .injected_fields
                .get("cwd")
                .map(String::as_str),
            Some("workspace_root")
        );
        let command_description = descriptor
            .model_input_schema
            .as_ref()
            .unwrap()
            .pointer("/properties/command/description")
            .and_then(Value::as_str)
            .unwrap();
        assert!(command_description.contains("Shell command by default"));
        assert!(descriptor
            .examples
            .iter()
            .any(|example| example.parameters == json!({"command": "rg -n \"TODO\" crates"})));
    }

    #[test]
    fn bounded_text_reports_truncation() {
        let small = b"abc";
        assert_eq!(bounded_text(small), ("abc".to_string(), false, 3));
        let large = vec![b'x'; OUTPUT_PREVIEW_CHARS + 10];
        let (text, truncated, total) = bounded_text(&large);
        assert_eq!(text.len(), OUTPUT_PREVIEW_CHARS);
        assert!(truncated);
        assert_eq!(total, OUTPUT_PREVIEW_CHARS + 10);
    }
}
