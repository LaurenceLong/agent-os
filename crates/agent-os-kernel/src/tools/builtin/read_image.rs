use super::{schema, BuiltinTool};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(in crate::tools) const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "read_image",
        descriptor,
        execute,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_read_image",
            name: "read_image",
            description: "Read a supported workspace image as a model-visible image attachment. Use this for PNG, JPEG, GIF, WEBP, BMP, TIFF, AVIF, or ICO files when the current model supports image input.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 1,
            input_schema: schema::object(
                &["workspace_root", "path"],
                json!({
                    "workspace_root": {"type": "string"},
                    "path": {"type": "string"}
                }),
            ),
            model_input_schema: schema::object(
                &["path"],
                json!({
                    "path": {"type": "string", "description": "Workspace-relative image path to read. Do not use absolute paths or '..'."}
                }),
            ),
            examples: vec![schema::example(
                "Read a screenshot image from the workspace.",
                json!({"path": "screenshots/failure.png"}),
                "Returns MIME type, byte count, and a data URL that the provider adapter projects as an image input.",
            )],
            output_schema: schema::object(
                &[
                    "tool",
                    "status",
                    "input",
                    "driver_class",
                    "path",
                    "mime_type",
                    "encoding",
                    "data_url",
                    "bytes_read",
                ],
                json!({
                    "tool": {"type": "string"},
                    "status": {"enum": ["ok"]},
                    "input": {"type": "object"},
                    "driver_class": {"type": "string"},
                    "path": {"type": "string"},
                    "mime_type": {"type": "string"},
                    "encoding": {"enum": ["base64"]},
                    "data_url": {"type": "string"},
                    "bytes_read": {"type": "integer"}
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
    super::super::driver::workspace::run_workspace_read_image(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_hides_workspace_root_and_describes_image_path() {
        let descriptor = descriptor("now");
        let model_schema = descriptor.model_input_schema.as_ref().unwrap();

        assert!(model_schema.pointer("/properties/path").is_some());
        assert!(model_schema.pointer("/properties/workspace_root").is_none());
        assert!(descriptor
            .examples
            .iter()
            .any(|example| { example.parameters == json!({"path": "screenshots/failure.png"}) }));
        assert_eq!(
            descriptor
                .runtime_input_policy
                .injected_fields
                .get("workspace_root")
                .map(String::as_str),
            Some("workspace_root")
        );
        assert_eq!(descriptor.evidence_type, Some(EvidenceType::SourceRef));
    }
}
