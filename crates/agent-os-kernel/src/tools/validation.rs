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
    Ok(())
}
