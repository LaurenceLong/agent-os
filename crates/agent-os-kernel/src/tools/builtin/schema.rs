use agent_os_sys::{
    EvidenceType, IdempotencyMode, ToolDescriptor, ToolDriverClass, ToolExample,
    ToolRuntimeInputPolicy,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) struct DescriptorSpec {
    pub(super) tool_id: &'static str,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) driver_class: ToolDriverClass,
    pub(super) risk_level: u8,
    pub(super) input_schema: Value,
    pub(super) model_input_schema: Value,
    pub(super) examples: Vec<ToolExample>,
    pub(super) output_schema: Value,
    pub(super) runtime_input_policy: ToolRuntimeInputPolicy,
    pub(super) idempotency: IdempotencyMode,
    pub(super) evidence_type: Option<EvidenceType>,
}

pub(super) fn descriptor(now: &str, spec: DescriptorSpec) -> ToolDescriptor {
    ToolDescriptor {
        tool_id: spec.tool_id.to_string(),
        name: spec.name.to_string(),
        description: spec.description.to_string(),
        version: "0.2.0".to_string(),
        driver_class: spec.driver_class,
        risk_level: spec.risk_level,
        input_schema: spec.input_schema,
        model_input_schema: Some(spec.model_input_schema),
        examples: spec.examples,
        output_schema: spec.output_schema,
        runtime_input_policy: spec.runtime_input_policy,
        idempotency: spec.idempotency,
        evidence_type: spec.evidence_type,
        created_at: now.to_string(),
        ..ToolDescriptor::default()
    }
}

pub(super) fn example(
    description: &'static str,
    parameters: Value,
    expected_result: &'static str,
) -> ToolExample {
    ToolExample {
        description: description.to_string(),
        parameters,
        expected_result: expected_result.to_string(),
    }
}

pub(super) fn object(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

pub(super) fn injected_workspace_root(field_name: &str) -> ToolRuntimeInputPolicy {
    let mut injected_fields = BTreeMap::new();
    injected_fields.insert(field_name.to_string(), "workspace_root".to_string());
    ToolRuntimeInputPolicy {
        injected_fields,
        ..ToolRuntimeInputPolicy::default()
    }
}

pub(super) fn required_scopes(scopes: &[&str]) -> ToolRuntimeInputPolicy {
    ToolRuntimeInputPolicy {
        required_resource_scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        ..ToolRuntimeInputPolicy::default()
    }
}

pub(super) fn permission_set_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "max_risk_level",
            "allowed_syscalls",
            "resource_scopes",
            "allowed_tool_names",
            "allowed_tool_driver_classes",
            "approval_required_above",
            "requires_evidence_for"
        ],
        "properties": {
            "max_risk_level": {"type": "integer", "minimum": 0, "maximum": 6},
            "allowed_syscalls": {"type": "array", "items": {"type": "string"}},
            "resource_scopes": {"type": "array", "items": {"type": "string"}},
            "allowed_tool_names": {"type": "array", "items": {"type": "string"}},
            "allowed_tool_driver_classes": {
                "type": "array",
                "items": {
                    "enum": [
                        "kernel_builtin",
                        "filesystem",
                        "shell",
                        "git",
                        "mcp",
                        "browser",
                        "external_api"
                    ]
                }
            },
            "approval_required_above": {"type": "integer", "minimum": 0, "maximum": 6},
            "requires_evidence_for": {"type": "array", "items": {"type": "string"}}
        },
        "additionalProperties": false
    })
}
