use super::api_error::ProviderApiError;
use super::audit::append_jsonl;
use super::messages::{build_anthropic_messages, build_messages};
use super::parser::{parse_anthropic_response, parse_response};
use super::prompt::default_system_prompt;
use super::tools::{anthropic_tool_definitions_for_request, tool_definitions_for_request};
use crate::{ModelClient, ModelTurnRequest, ModelTurnResponse};
use agent_os_sys::LlmApiStyle;
use agent_os_sys::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MAX_TOKENS: u64 = 4096;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct OpenAiModelClient {
    api_base: String,
    api_key: String,
    model: String,
    max_tokens: u64,
    temperature: Option<f64>,
    model_options: BTreeMap<String, Value>,
    request_timeout: Duration,
    system_prompt_override: Option<String>,
    api_style: LlmApiStyle,
    audit_log_path: Option<PathBuf>,
}

impl OpenAiModelClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: DEFAULT_API_BASE.to_string(),
            api_key: api_key.into(),
            model: model.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            temperature: None,
            model_options: BTreeMap::new(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            system_prompt_override: None,
            api_style: LlmApiStyle::OpenAiCompatible,
            audit_log_path: None,
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self.api_style = LlmApiStyle::from_base_url(&self.api_base);
        self
    }

    pub fn with_api_style(mut self, api_style: LlmApiStyle) -> Self {
        self.api_style = api_style;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_model_options(mut self, options: BTreeMap<String, Value>) -> Self {
        self.model_options = options;
        self
    }

    pub fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt_override = Some(prompt.into());
        self
    }

    pub fn with_audit_log(mut self, path: impl Into<PathBuf>) -> Self {
        self.audit_log_path = Some(path.into());
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn api_style(&self) -> LlmApiStyle {
        self.api_style
    }
}

impl ModelClient for OpenAiModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        match self.api_style {
            LlmApiStyle::OpenAiCompatible => self.next_openai_compatible(request),
            LlmApiStyle::AnthropicCompatible => self.next_anthropic_compatible(request),
        }
    }
}

impl OpenAiModelClient {
    fn next_openai_compatible(
        &mut self,
        request: &ModelTurnRequest,
    ) -> AgentOsResult<ModelTurnResponse> {
        let workspace_root = request.workspace_root.to_string_lossy().to_string();
        let messages = build_messages(request, &workspace_root, &self.system_prompt_override);
        let tools = tool_definitions_for_request(request);

        let mut body = self.model_options_body();
        body.insert("model".to_string(), json!(self.model.clone()));
        body.insert("messages".to_string(), Value::Array(messages));
        body.insert("tools".to_string(), Value::Array(tools));
        body.insert("tool_choice".to_string(), json!("auto"));
        body.insert("max_tokens".to_string(), json!(self.max_tokens));
        if let Some(temperature) = self.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }
        let body = Value::Object(body);

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let bearer = format!("Bearer {}", self.api_key);
        self.audit_provider_event(json!({
            "type": "provider_request",
            "provider": "openai-compatible",
            "endpoint": "/chat/completions",
            "body": body.clone(),
        }))?;

        let response_body = match ureq::post(&url)
            .timeout(self.request_timeout)
            .set("Authorization", &bearer)
            .set("Content-Type", "application/json")
            .send_json(&body)
        {
            Ok(response) => response.into_json::<Value>().map_err(|e| {
                AgentOsError::Validation(format!("failed to parse API response: {e}"))
            })?,
            Err(ureq::Error::Status(code, response)) => {
                let retry_after_ms = response.header("retry-after-ms").map(str::to_string);
                let retry_after = response.header("retry-after").map(str::to_string);
                let error_text = response.into_string().unwrap_or_default();
                let error = ProviderApiError::from_status(
                    "openai-compatible",
                    code,
                    error_text,
                    retry_after_ms.as_deref(),
                    retry_after.as_deref(),
                );
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
            Err(e) => {
                let error = ProviderApiError::from_transport("openai-compatible", &e);
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
        };

        self.audit_provider_event(json!({
            "type": "provider_response",
            "provider": "openai-compatible",
            "body": response_body.clone(),
        }))?;
        let parsed = parse_response(&response_body, request)?;
        self.audit_provider_event(json!({
            "type": "parsed_model_response",
            "provider": "openai-compatible",
            "response": parsed.clone(),
        }))?;
        Ok(parsed)
    }

    fn next_anthropic_compatible(
        &mut self,
        request: &ModelTurnRequest,
    ) -> AgentOsResult<ModelTurnResponse> {
        let workspace_root = request.workspace_root.to_string_lossy().to_string();
        let mut body = self.model_options_body();
        body.insert("model".to_string(), json!(self.model.clone()));
        body.insert(
            "system".to_string(),
            json!(self
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
        body.insert("max_tokens".to_string(), json!(self.max_tokens));
        if let Some(temperature) = self.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }
        let body = Value::Object(body);

        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));
        let bearer = format!("Bearer {}", self.api_key);
        self.audit_provider_event(json!({
            "type": "provider_request",
            "provider": "anthropic-compatible",
            "endpoint": "/v1/messages",
            "body": body.clone(),
        }))?;
        let response_body = match ureq::post(&url)
            .timeout(self.request_timeout)
            .set("x-api-key", &self.api_key)
            .set("Authorization", &bearer)
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(&body)
        {
            Ok(response) => response.into_json::<Value>().map_err(|e| {
                AgentOsError::Validation(format!("failed to parse API response: {e}"))
            })?,
            Err(ureq::Error::Status(code, response)) => {
                let retry_after_ms = response.header("retry-after-ms").map(str::to_string);
                let retry_after = response.header("retry-after").map(str::to_string);
                let error_text = response.into_string().unwrap_or_default();
                let error = ProviderApiError::from_status(
                    "anthropic-compatible",
                    code,
                    error_text,
                    retry_after_ms.as_deref(),
                    retry_after.as_deref(),
                );
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
            Err(e) => {
                let error = ProviderApiError::from_transport("anthropic-compatible", &e);
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
        };

        self.audit_provider_event(json!({
            "type": "provider_response",
            "provider": "anthropic-compatible",
            "body": response_body.clone(),
        }))?;
        let parsed = parse_anthropic_response(&response_body, request)?;
        self.audit_provider_event(json!({
            "type": "parsed_model_response",
            "provider": "anthropic-compatible",
            "response": parsed.clone(),
        }))?;
        Ok(parsed)
    }

    fn audit_provider_event(&self, entry: Value) -> AgentOsResult<()> {
        let Some(path) = &self.audit_log_path else {
            return Ok(());
        };
        append_jsonl(path, &entry)
    }

    fn model_options_body(&self) -> Map<String, Value> {
        self.model_options
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_timeout_defaults_and_can_be_overridden() {
        let client = OpenAiModelClient::new("test-key", "test-model");
        assert_eq!(client.request_timeout, Duration::from_secs(120));

        let client = OpenAiModelClient::new("test-key", "test-model")
            .with_request_timeout(Duration::from_secs(30));
        assert_eq!(client.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn model_options_seed_request_body_without_overriding_runtime_fields() {
        let client =
            OpenAiModelClient::new("test-key", "wire-model").with_model_options(BTreeMap::from([
                ("reasoningEffort".to_string(), json!("high")),
                ("max_tokens".to_string(), json!(999999)),
            ]));

        let body = client.model_options_body();

        assert_eq!(body["reasoningEffort"], json!("high"));
        assert_eq!(body["max_tokens"], json!(999999));
    }
}
