//! Built-in tool registry.
//!
//! Each model-visible built-in tool owns its descriptor and schema in its
//! matching module. Execution may delegate into a focused driver family, but
//! dispatch and public contract ownership live here.

mod accomplish_goal;
mod agent_control;
mod apply_patch;
mod ask_human;
pub(super) mod glob_files;
pub(super) mod grep_files;
mod load_skill;
mod post_blackboard;
pub(super) mod read_file;
pub(super) mod read_image;
mod read_skill_resource;
mod record_evidence;
mod report_supervisor;
mod request_permissions;
pub(super) mod run_command;
mod schema;
mod set_goal;
mod submit_final;
mod tool_search;
mod update_checklist;
mod write_stdin;

use crate::*;
use agent_os_sys::*;
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) struct BuiltinTool {
    pub name: &'static str,
    pub descriptor: fn(&str) -> ToolDescriptor,
    pub execute:
        fn(&Kernel, &SyscallEnvelope, &ToolDescriptor, &str, &Value) -> AgentOsResult<Value>,
}

pub(crate) fn core_tool_descriptors(now: &str) -> Vec<ToolDescriptor> {
    all_tools()
        .into_iter()
        .map(|tool| (tool.descriptor)(now))
        .collect()
}

pub(super) fn tool(name: &str) -> Option<BuiltinTool> {
    all_tools().into_iter().find(|tool| tool.name == name)
}

fn all_tools() -> Vec<BuiltinTool> {
    vec![
        apply_patch::tool(),
        glob_files::tool(),
        grep_files::tool(),
        read_file::tool(),
        read_image::tool(),
        run_command::tool(),
        write_stdin::tool(),
        set_goal::tool(),
        accomplish_goal::tool(),
        update_checklist::tool(),
        record_evidence::tool(),
        report_supervisor::tool(),
        post_blackboard::tool(),
        ask_human::tool(),
        request_permissions::tool(),
        load_skill::tool(),
        read_skill_resource::tool(),
        submit_final::tool(),
        tool_search::tool(),
        agent_control::tool(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_descriptor_has_examples() {
        let descriptors = core_tool_descriptors("now");

        assert_eq!(descriptors.len(), all_tools().len());
        for descriptor in descriptors {
            assert_eq!(
                descriptor.lifecycle.foreground_timeout_ms, DEFAULT_TOOL_FOREGROUND_TIMEOUT_MS,
                "{} must declare the default foreground timeout",
                descriptor.name
            );
            assert_eq!(
                descriptor.lifecycle.background_execution,
                ToolBackgroundExecution::KernelWorker,
                "{} must declare kernel-worker background execution",
                descriptor.name
            );
            assert_eq!(
                descriptor.lifecycle.recovery,
                ToolRecoveryPolicy::CancelOrphanRunning,
                "{} must declare orphan-running recovery behavior",
                descriptor.name
            );
            assert_eq!(
                descriptor.lifecycle.output_management.mode,
                ToolOutputManagementMode::ManagedTextFields,
                "{} must declare managed text output behavior",
                descriptor.name
            );
            assert_eq!(
                descriptor.lifecycle.output_management.max_window_bytes,
                TOOL_OUTPUT_MAX_WINDOW_BYTES,
                "{} must declare the managed output byte window",
                descriptor.name
            );
            assert!(
                !descriptor.examples.is_empty(),
                "{} must define at least one example in its owner file",
                descriptor.name
            );
            for example in &descriptor.examples {
                assert!(
                    example.parameters.is_object(),
                    "{} example parameters must be an object",
                    descriptor.name
                );
                assert!(
                    !example.description.trim().is_empty(),
                    "{} example description must be present",
                    descriptor.name
                );
                assert!(
                    !example.expected_result.trim().is_empty(),
                    "{} example expected_result must be present",
                    descriptor.name
                );
            }
            if let Some(model_input_schema) = &descriptor.model_input_schema {
                let required_fields = model_input_schema
                    .pointer("/required")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str);
                for field in required_fields {
                    assert!(
                        descriptor
                            .examples
                            .iter()
                            .any(|example| example.parameters.get(field).is_some()),
                        "{} required model_input_schema field `{}` must appear in at least one example",
                        descriptor.name,
                        field
                    );
                }
            }
        }
    }
}
