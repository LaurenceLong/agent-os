//! Built-in tool registry.
//!
//! Each model-visible built-in tool owns its descriptor and schema in its
//! matching module. Execution may delegate into a focused driver family, but
//! dispatch and public contract ownership live here.

mod accomplish_goal;
mod agent_control;
mod apply_patch;
mod ask_human;
mod load_skill;
mod post_blackboard;
pub(super) mod read_file;
mod read_skill_resource;
mod record_evidence;
mod report_supervisor;
mod request_permissions;
pub(super) mod run_command;
mod schema;
mod set_goal;
mod submit_final;
mod update_checklist;

use crate::*;
use agent_os_sys::*;
use serde_json::Value;
use std::time::Duration;

pub(super) const FOREGROUND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
pub(super) struct BuiltinTool {
    pub name: &'static str,
    pub descriptor: fn(&str) -> ToolDescriptor,
    pub execute:
        fn(&Kernel, &SyscallEnvelope, &ToolDescriptor, &str, &Value) -> AgentOsResult<Value>,
    pub foreground_timeout: Duration,
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
        read_file::tool(),
        run_command::tool(),
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
        }
    }
}
