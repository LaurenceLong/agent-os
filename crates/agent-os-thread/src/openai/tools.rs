use serde_json::{json, Value};

use crate::ModelTurnRequest;
#[cfg(test)]
use agent_os_kernel::Kernel;
#[cfg(test)]
use agent_os_sys::AgentControlBlock;
use agent_os_sys::{PermissionSet, ToolDescriptor};

#[cfg(test)]
pub(crate) fn tool_definitions() -> Vec<Value> {
    openai_tools_from_descriptors(&default_core_descriptors())
}

#[cfg(test)]
pub(crate) fn tool_definitions_for_thread(thread: &AgentControlBlock) -> Vec<Value> {
    let descriptors = default_core_descriptors();
    let mut tools = openai_tools_from_descriptors(
        &descriptors
            .iter()
            .filter(|descriptor| {
                descriptor_permission_allows(descriptor, &thread.effective_permissions_snapshot)
            })
            .cloned()
            .collect::<Vec<_>>(),
    );
    redact_control_plane_tools(&mut tools, thread.security_level.allows_control_plane());
    redact_privileged_agent_control_actions(
        &mut tools,
        thread.security_level.allows_control_plane(),
    );
    tools
}

pub(crate) fn tool_definitions_for_request(request: &ModelTurnRequest) -> Vec<Value> {
    let mut tools = openai_tools_from_descriptors(
        &request
            .context
            .tool_descriptors
            .iter()
            .filter(|descriptor| {
                descriptor_permission_allows(
                    descriptor,
                    &request.thread.effective_permissions_snapshot,
                )
            })
            .cloned()
            .collect::<Vec<_>>(),
    );
    redact_control_plane_tools(
        &mut tools,
        request.thread.security_level.allows_control_plane(),
    );
    redact_privileged_agent_control_actions(
        &mut tools,
        request.thread.security_level.allows_control_plane(),
    );
    tools
}

#[cfg(test)]
pub(crate) fn anthropic_tool_definitions() -> Vec<Value> {
    openai_tools_to_anthropic(tool_definitions())
}

#[cfg(test)]
pub(crate) fn anthropic_tool_definitions_for_thread(thread: &AgentControlBlock) -> Vec<Value> {
    openai_tools_to_anthropic(tool_definitions_for_thread(thread))
}

pub(crate) fn anthropic_tool_definitions_for_request(request: &ModelTurnRequest) -> Vec<Value> {
    openai_tools_to_anthropic(tool_definitions_for_request(request))
}

fn openai_tools_from_descriptors(descriptors: &[ToolDescriptor]) -> Vec<Value> {
    descriptors
        .iter()
        .filter_map(descriptor_to_openai_tool)
        .collect()
}

fn descriptor_to_openai_tool(descriptor: &ToolDescriptor) -> Option<Value> {
    if descriptor.description.is_empty() {
        return None;
    }
    let parameters = descriptor.model_input_schema.clone()?;
    Some(json!({
        "type": "function",
        "function": {
            "name": descriptor.name,
            "description": descriptor.description,
            "parameters": parameters
        }
    }))
}

fn openai_tools_to_anthropic(tools: Vec<Value>) -> Vec<Value> {
    tools
        .into_iter()
        .filter_map(|tool| {
            let function = tool.get("function")?;
            Some(json!({
                "name": function.get("name")?.clone(),
                "description": function.get("description")?.clone(),
                "input_schema": function.get("parameters")?.clone()
            }))
        })
        .collect()
}

fn descriptor_permission_allows(descriptor: &ToolDescriptor, permissions: &PermissionSet) -> bool {
    let name_allowed = permissions
        .allowed_tool_names
        .iter()
        .any(|allowed| allowed == "*" || allowed == &descriptor.name);
    let driver_allowed = permissions
        .allowed_tool_driver_classes
        .contains(&descriptor.driver_class);
    let scoped_driver_allowed = driver_allowed
        && !descriptor
            .runtime_input_policy
            .required_resource_scopes
            .is_empty()
        && descriptor
            .runtime_input_policy
            .required_resource_scopes
            .iter()
            .all(|scope| scope_list_allows(&permissions.resource_scopes, scope));
    name_allowed || scoped_driver_allowed
}

fn redact_privileged_agent_control_actions(tools: &mut [Value], include: bool) {
    if include {
        return;
    }
    for tool in tools {
        let name = tool
            .get("function")
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str);
        if name != Some("agent_control") {
            continue;
        }
        if let Some(actions) = tool
            .pointer_mut("/function/parameters/properties/action/enum")
            .and_then(Value::as_array_mut)
        {
            actions.retain(|action| {
                action.as_str().is_none_or(|action| {
                    !matches!(action, "kill" | "delete_session" | "purge_state")
                })
            });
        }
    }
}

fn redact_control_plane_tools(tools: &mut Vec<Value>, include: bool) {
    if include {
        return;
    }
    tools.retain(|tool| {
        tool.pointer("/function/name")
            .and_then(Value::as_str)
            .is_none_or(|name| !matches!(name, "agent_control" | "set_goal"))
    });
}

fn scope_list_allows(allowed_scopes: &[String], requested_scope: &str) -> bool {
    allowed_scopes
        .iter()
        .any(|allowed| scope_pattern_allows(allowed, requested_scope))
}

fn scope_pattern_allows(allowed: &str, requested: &str) -> bool {
    if allowed == "*" || allowed == requested {
        return true;
    }
    allowed.strip_suffix(":*").is_some_and(|prefix| {
        requested == prefix
            || requested
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(':'))
    })
}

#[cfg(test)]
fn default_core_descriptors() -> Vec<ToolDescriptor> {
    let mut descriptors: Vec<_> = Kernel::new()
        .state_snapshot()
        .unwrap()
        .tool_descriptors
        .values()
        .cloned()
        .collect();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    descriptors
}
