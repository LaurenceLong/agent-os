use super::{schema, BuiltinTool, FOREGROUND_TIMEOUT};
use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

pub(super) fn tool() -> BuiltinTool {
    BuiltinTool {
        name: "apply_patch",
        descriptor,
        execute,
        foreground_timeout: FOREGROUND_TIMEOUT,
    }
}

fn descriptor(now: &str) -> ToolDescriptor {
    schema::descriptor(
        now,
        schema::DescriptorSpec {
            tool_id: "tool_apply_patch",
            name: "apply_patch",
            description: "Apply exactly one workspace file patch between *** Begin Patch and *** End Patch. Use *** Add File: path, *** Update File: path, or *** Delete File: path. Update hunks accept plain context lines or canonical leading-space context lines with -old and +new changes.",
            driver_class: ToolDriverClass::Filesystem,
            risk_level: 4,
            input_schema: schema::object(
            &["workspace_root", "patch"],
            json!({
                "workspace_root": {"type": "string"},
                "patch": {"type": "string"}
            }),
        ),
            model_input_schema: schema::object(
            &["patch"],
            json!({
                "patch": {
                    "type": "string",
                    "description": "Patch document with *** Begin Patch and *** End Patch. Add files with: *** Add File: path then +content lines. Update files with: *** Update File: path then @@ hunks; unchanged context may be plain lines or lines prefixed with one space, changed lines use -old and +new. Delete files with: *** Delete File: path."
                }
            }),
        ),
            examples: vec![
                schema::example(
                    "Add a new file; every content line must start with +.",
                    json!({
                        "patch": "*** Begin Patch\n*** Add File: docs/example.md\n+# Example\n+\n+Created by apply_patch.\n*** End Patch\n"
                    }),
                    "Creates one bounded workspace file and returns diff metadata.",
                ),
                schema::example(
                    "Update an existing file; keep exact context lines and mark removed/added lines with - and +.",
                    json!({
                        "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\nfn answer() -> i32 {\n-    1\n+    2\n}\n*** End Patch\n"
                    }),
                    "Applies one bounded workspace file update and returns diff metadata.",
                ),
                schema::example(
                    "Delete an obsolete file with no hunks.",
                    json!({
                        "patch": "*** Begin Patch\n*** Delete File: tmp/obsolete.txt\n*** End Patch\n"
                    }),
                    "Deletes one bounded workspace file and returns diff metadata.",
                ),
            ],
            output_schema: schema::object(
            &["tool", "status", "input", "driver_class", "operation", "path", "truncated"],
            json!({
                "tool": {"type": "string"},
                "status": {"enum": ["ok"]},
                "input": {"type": "object"},
                "driver_class": {"type": "string"},
                "operation": {"enum": ["create", "update", "delete"]},
                "path": {"type": "string"},
                "created_path": {"type": "string"},
                "changed_path": {"type": "string"},
                "deleted_path": {"type": "string"},
                "bytes_written": {"type": "integer"},
                "deleted_bytes": {"type": "integer"},
                "replacements": {"type": "integer"},
                "before_hash": {"type": "string"},
                "after_hash": {"type": "string"},
                "preview": {"type": "string"},
                "truncated": {"type": "boolean"}
            }),
        ),
            runtime_input_policy: schema::injected_workspace_root("workspace_root"),
            idempotency: IdempotencyMode::KernelDeduplicated,
            evidence_type: Some(EvidenceType::DiffRef),
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
    super::super::driver::workspace::run_workspace_apply_patch(kernel, syscall, descriptor, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_patch_and_injects_workspace_root() {
        let descriptor = descriptor("now");
        assert_eq!(
            descriptor
                .model_input_schema
                .as_ref()
                .unwrap()
                .pointer("/required/0")
                .and_then(Value::as_str),
            Some("patch")
        );
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
    fn examples_cover_add_update_and_delete_patch_shapes() {
        let descriptor = descriptor("now");
        let patches = descriptor
            .examples
            .iter()
            .map(|example| {
                example
                    .parameters
                    .get("patch")
                    .and_then(Value::as_str)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        assert_eq!(patches.len(), 3);
        assert!(patches
            .iter()
            .all(|patch| patch.starts_with("*** Begin Patch\n")));
        assert!(patches
            .iter()
            .all(|patch| patch.ends_with("*** End Patch\n")));
        assert!(patches.iter().any(|patch| patch.contains("*** Add File:")));
        assert!(patches
            .iter()
            .any(|patch| patch.contains("*** Update File:")));
        assert!(patches
            .iter()
            .any(|patch| patch.contains("*** Delete File:")));
        assert!(patches
            .iter()
            .any(|patch| patch.contains("\n+Created by apply_patch.")));
        assert!(patches
            .iter()
            .any(|patch| patch.contains("\n-    1\n+    2\n")));
    }
}
