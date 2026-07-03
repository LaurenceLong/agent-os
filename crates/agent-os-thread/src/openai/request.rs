use super::messages::{build_anthropic_messages, build_messages};
use super::prompt::default_system_prompt;
use super::tools::{anthropic_tool_definitions_for_request, tool_definitions_for_request};
use crate::ModelTurnRequest;
use agent_os_sys::{AgentOsError, AgentOsResult, LlmApiStyle};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) enum ResponseParser {
    OpenAiChatCompletions,
    OpenAiResponses,
    AnthropicMessages,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderRequest {
    pub provider_label: &'static str,
    pub endpoint_path: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Value,
    pub parser: ResponseParser,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderRequestConfig<'a> {
    pub endpoint: LlmApiStyle,
    pub api_base: &'a str,
    pub api_key: &'a str,
    pub model: &'a str,
    pub max_tokens: u64,
    pub temperature: Option<f64>,
    pub model_options: &'a BTreeMap<String, Value>,
    pub system_prompt_override: &'a Option<String>,
}

pub(crate) fn build_provider_request(
    config: ProviderRequestConfig<'_>,
    request: &ModelTurnRequest,
) -> AgentOsResult<ProviderRequest> {
    match config.endpoint {
        LlmApiStyle::OpenAiChatCompletions => openai_chat_completions(config, request),
        LlmApiStyle::OpenAiResponses => openai_responses(config, request),
        LlmApiStyle::AnthropicMessages => anthropic_messages(config, request),
    }
}

fn openai_chat_completions(
    config: ProviderRequestConfig<'_>,
    request: &ModelTurnRequest,
) -> AgentOsResult<ProviderRequest> {
    let workspace_root = request.workspace_root.to_string_lossy().to_string();
    let mut body = model_options_body(config.model_options);
    body.insert("model".to_string(), json!(config.model));
    body.insert(
        "messages".to_string(),
        Value::Array(build_messages(
            request,
            &workspace_root,
            config.system_prompt_override,
        )),
    );
    body.insert(
        "tools".to_string(),
        Value::Array(tool_definitions_for_request(request)),
    );
    body.insert("tool_choice".to_string(), json!("auto"));
    body.insert("max_tokens".to_string(), json!(config.max_tokens));
    if request.model_capabilities.temperature {
        if let Some(temperature) = config.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }
    }
    let headers = vec![
        ("Authorization", format!("Bearer {}", config.api_key)),
        ("Content-Type", "application/json".to_string()),
    ];
    Ok(provider_request(
        config,
        LlmApiStyle::OpenAiChatCompletions,
        headers,
        Value::Object(body),
        ResponseParser::OpenAiChatCompletions,
    ))
}

fn openai_responses(
    config: ProviderRequestConfig<'_>,
    request: &ModelTurnRequest,
) -> AgentOsResult<ProviderRequest> {
    let workspace_root = request.workspace_root.to_string_lossy().to_string();
    let mut body = model_options_body(config.model_options);
    body.insert("model".to_string(), json!(config.model));
    body.insert(
        "input".to_string(),
        Value::Array(build_messages(
            request,
            &workspace_root,
            config.system_prompt_override,
        )),
    );
    body.insert(
        "tools".to_string(),
        Value::Array(openai_responses_tools(request)?),
    );
    body.insert("tool_choice".to_string(), json!("auto"));
    body.insert("max_output_tokens".to_string(), json!(config.max_tokens));
    if request.model_capabilities.temperature {
        if let Some(temperature) = config.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }
    }
    let headers = vec![
        ("Authorization", format!("Bearer {}", config.api_key)),
        ("Content-Type", "application/json".to_string()),
    ];
    Ok(provider_request(
        config,
        LlmApiStyle::OpenAiResponses,
        headers,
        Value::Object(body),
        ResponseParser::OpenAiResponses,
    ))
}

fn anthropic_messages(
    config: ProviderRequestConfig<'_>,
    request: &ModelTurnRequest,
) -> AgentOsResult<ProviderRequest> {
    let workspace_root = request.workspace_root.to_string_lossy().to_string();
    let mut body = model_options_body(config.model_options);
    body.insert("model".to_string(), json!(config.model));
    body.insert(
        "system".to_string(),
        json!(config
            .system_prompt_override
            .clone()
            .unwrap_or_else(|| default_system_prompt(request, &workspace_root))),
    );
    body.insert(
        "messages".to_string(),
        Value::Array(build_anthropic_messages(request, &workspace_root)),
    );
    body.insert(
        "tools".to_string(),
        Value::Array(anthropic_tool_definitions_for_request(request)),
    );
    body.insert("tool_choice".to_string(), json!({"type": "auto"}));
    body.insert("max_tokens".to_string(), json!(config.max_tokens));
    if request.model_capabilities.temperature {
        if let Some(temperature) = config.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }
    }
    let headers = vec![
        ("x-api-key", config.api_key.to_string()),
        ("Authorization", format!("Bearer {}", config.api_key)),
        ("anthropic-version", "2023-06-01".to_string()),
        ("Content-Type", "application/json".to_string()),
    ];
    Ok(provider_request(
        config,
        LlmApiStyle::AnthropicMessages,
        headers,
        Value::Object(body),
        ResponseParser::AnthropicMessages,
    ))
}

fn provider_request(
    config: ProviderRequestConfig<'_>,
    endpoint: LlmApiStyle,
    headers: Vec<(&'static str, String)>,
    body: Value,
    parser: ResponseParser,
) -> ProviderRequest {
    ProviderRequest {
        provider_label: endpoint.provider_label(),
        endpoint_path: endpoint.request_path(),
        url: format!(
            "{}{}",
            config.api_base.trim_end_matches('/'),
            endpoint.request_path()
        ),
        headers,
        body,
        parser,
    }
}

fn model_options_body(options: &BTreeMap<String, Value>) -> Map<String, Value> {
    options
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn openai_responses_tools(request: &ModelTurnRequest) -> AgentOsResult<Vec<Value>> {
    tool_definitions_for_request(request)
        .into_iter()
        .map(|tool| {
            let function = tool.get("function").ok_or_else(|| {
                AgentOsError::Validation("OpenAI tool missing function".to_string())
            })?;
            Ok(json!({
                "type": "function",
                "name": function.get("name").cloned().unwrap_or(Value::Null),
                "description": function.get("description").cloned().unwrap_or(Value::Null),
                "parameters": function.get("parameters").cloned().unwrap_or(Value::Null),
                "strict": false
            }))
        })
        .collect()
}
