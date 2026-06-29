use agent_os_sys::*;
use serde_json::json;

pub(super) fn default_routing_policy(now: &str) -> RoutingPolicy {
    RoutingPolicy {
        routing_policy_id: "route_default".to_string(),
        status: ProfileStatus::Active,
        name: "DefaultRouting".to_string(),
        rules: vec![
            json!({"when": {"role": "WorkerAgent"}, "use": {"model_alias": "coding-primary"}}),
            json!({"when": {"role": "ReviewerAgent"}, "use": {"model_alias": "review-primary"}}),
        ],
        created_at: now.to_string(),
        updated_at: now.to_string(),
        superseded_by: None,
    }
}

pub(super) fn default_provider_profile(now: &str) -> ProviderProfile {
    ProviderProfile {
        provider_profile_id: "prov_default".to_string(),
        status: ProfileStatus::Active,
        name: "DefaultProviderProfile".to_string(),
        default_provider_id: Some("primary-provider".to_string()),
        default_model_alias: Some("general-primary".to_string()),
        routing_policy_id: "route_default".to_string(),
        allowed_model_aliases: vec![
            "coding-primary".to_string(),
            "review-primary".to_string(),
            "general-primary".to_string(),
            "text-only".to_string(),
        ],
        credential_ref: CredentialRef {
            credential_ref_id: "cred_default_llm".to_string(),
            source: CredentialSource::LocalConfig,
            name: "default".to_string(),
        },
        retry_policy: Some(json!({
            "max_attempts": 2,
            "backoff_ms": 0
        })),
        transform_policy: Some(json!({
            "adapter_style": "openai-compatible"
        })),
        reasoning_defaults: json!({}),
        tool_visibility_profile: None,
        timeout_ms: Some(120_000),
        max_output_tokens: Some(16_000),
        created_at: now.to_string(),
        updated_at: now.to_string(),
        superseded_by: None,
    }
}

pub(super) fn strict_text_provider_profile(now: &str) -> ProviderProfile {
    ProviderProfile {
        provider_profile_id: "prov_strict_text".to_string(),
        status: ProfileStatus::Active,
        name: "StrictTextProviderProfile".to_string(),
        default_provider_id: Some("primary-provider".to_string()),
        default_model_alias: Some("text-only".to_string()),
        routing_policy_id: "route_default".to_string(),
        allowed_model_aliases: vec!["text-only".to_string()],
        credential_ref: CredentialRef {
            credential_ref_id: "cred_strict_text_llm".to_string(),
            source: CredentialSource::LocalConfig,
            name: "default".to_string(),
        },
        retry_policy: Some(json!({
            "max_attempts": 1,
            "backoff_ms": 0
        })),
        transform_policy: Some(json!({
            "adapter_style": "openai-compatible"
        })),
        reasoning_defaults: json!({}),
        tool_visibility_profile: None,
        timeout_ms: Some(120_000),
        max_output_tokens: Some(16_000),
        created_at: now.to_string(),
        updated_at: now.to_string(),
        superseded_by: None,
    }
}

pub(super) fn core_model_aliases(now: &str) -> Vec<ModelAlias> {
    [
        (
            "coding-primary",
            "primary-provider",
            "primary-coding-model",
            true,
            true,
        ),
        (
            "review-primary",
            "primary-provider",
            "primary-review-model",
            true,
            true,
        ),
        (
            "general-primary",
            "primary-provider",
            "primary-general-model",
            true,
            true,
        ),
        (
            "text-only",
            "primary-provider",
            "primary-text-model",
            true,
            false,
        ),
    ]
    .into_iter()
    .map(|alias| ModelAlias {
        model_alias_id: new_id("alias_"),
        alias: alias.0.to_string(),
        provider_id: alias.1.to_string(),
        provider_model_name: alias.2.to_string(),
        capabilities: json!({
            "streaming": alias.3,
            "tool_calling": true,
            "reasoning": true,
            "image_input": false,
            "structured_output": alias.4
        }),
        limits: json!({
            "context_window": 128000,
            "max_output_tokens": 16000
        }),
        cost: json!({
            "input_per_1m": null,
            "output_per_1m": null
        }),
        status: "Active".to_string(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    })
    .collect()
}
