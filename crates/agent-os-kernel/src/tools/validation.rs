use agent_os_sys::*;

pub(super) fn validate_tool_descriptor(descriptor: &ToolDescriptor) -> AgentOsResult<()> {
    if descriptor.tool_id.is_empty()
        || descriptor.name.is_empty()
        || descriptor.version.is_empty()
        || !descriptor.input_schema.is_object()
        || !descriptor.output_schema.is_object()
    {
        return Err(AgentOsError::Validation(
            "tool descriptor must include id, name, version, input schema, and output schema"
                .to_string(),
        ));
    }
    if descriptor.lifecycle.foreground_timeout_ms == 0 {
        return Err(AgentOsError::Validation(
            "tool descriptor lifecycle foreground timeout must be greater than zero".to_string(),
        ));
    }
    let output = &descriptor.lifecycle.output_management;
    if output.default_new_lines == 0
        || output.default_page_lines == 0
        || output.max_lines == 0
        || output.max_window_bytes == 0
        || output.default_new_lines > output.max_lines
        || output.default_page_lines > output.max_lines
    {
        return Err(AgentOsError::Validation(
            "tool descriptor lifecycle output management limits are invalid".to_string(),
        ));
    }
    Ok(())
}
