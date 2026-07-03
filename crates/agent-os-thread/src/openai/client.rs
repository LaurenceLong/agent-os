use super::api_error::ProviderApiError;
use super::audit::append_jsonl;
use super::parser::{parse_anthropic_response, parse_openai_responses_response, parse_response};
use super::request::{build_provider_request, ProviderRequestConfig, ResponseParser};
use crate::{ModelClient, ModelTurnRequest, ModelTurnResponse};
use agent_os_sys::LlmApiStyle;
use agent_os_sys::*;
use serde_json::{json, Value};
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
    endpoint: LlmApiStyle,
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
            endpoint: LlmApiStyle::OpenAiChatCompletions,
            audit_log_path: None,
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    pub fn with_endpoint(mut self, endpoint: LlmApiStyle) -> Self {
        self.endpoint = endpoint;
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

    pub fn endpoint(&self) -> LlmApiStyle {
        self.endpoint
    }
}

impl ModelClient for OpenAiModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        let provider_request = build_provider_request(
            ProviderRequestConfig {
                endpoint: self.endpoint,
                api_base: &self.api_base,
                api_key: &self.api_key,
                model: &self.model,
                max_tokens: self.max_tokens,
                temperature: self.temperature,
                model_options: &self.model_options,
                system_prompt_override: &self.system_prompt_override,
            },
            request,
        )?;
        self.audit_provider_event(json!({
            "type": "provider_request",
            "provider": provider_request.provider_label,
            "endpoint": provider_request.endpoint_path,
            "body": provider_request.body.clone(),
        }))?;

        let mut call = ureq::post(&provider_request.url).timeout(self.request_timeout);
        for (name, value) in &provider_request.headers {
            call = call.set(name, value);
        }
        let response_body = match call.send_json(&provider_request.body) {
            Ok(response) => response.into_json::<Value>().map_err(|error| {
                AgentOsError::Validation(format!("failed to parse API response: {error}"))
            })?,
            Err(ureq::Error::Status(code, response)) => {
                let retry_after_ms = response.header("retry-after-ms").map(str::to_string);
                let retry_after = response.header("retry-after").map(str::to_string);
                let error_text = response.into_string().unwrap_or_default();
                let error = ProviderApiError::from_status(
                    provider_request.provider_label,
                    code,
                    error_text,
                    retry_after_ms.as_deref(),
                    retry_after.as_deref(),
                );
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
            Err(error) => {
                let error =
                    ProviderApiError::from_transport(provider_request.provider_label, &error);
                self.audit_provider_event(error.to_audit_event())?;
                return Err(error.into_agent_error());
            }
        };

        self.audit_provider_event(json!({
            "type": "provider_response",
            "provider": provider_request.provider_label,
            "body": response_body.clone(),
        }))?;
        let parsed = match provider_request.parser {
            ResponseParser::OpenAiChatCompletions => parse_response(&response_body, request),
            ResponseParser::OpenAiResponses => {
                parse_openai_responses_response(&response_body, request)
            }
            ResponseParser::AnthropicMessages => parse_anthropic_response(&response_body, request),
        }?;
        self.audit_provider_event(json!({
            "type": "parsed_model_response",
            "provider": provider_request.provider_label,
            "response": parsed.clone(),
        }))?;
        Ok(parsed)
    }
}

impl OpenAiModelClient {
    fn audit_provider_event(&self, entry: Value) -> AgentOsResult<()> {
        let Some(path) = &self.audit_log_path else {
            return Ok(());
        };
        append_jsonl(path, &entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openai::tests::support::make_request;

    #[test]
    fn request_timeout_defaults_and_can_be_overridden() {
        let client = OpenAiModelClient::new("test-key", "test-model");
        assert_eq!(client.request_timeout, Duration::from_secs(120));

        let client = OpenAiModelClient::new("test-key", "test-model")
            .with_request_timeout(Duration::from_secs(30));
        assert_eq!(client.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn model_options_seed_request_body_but_runtime_fields_stay_authoritative() {
        let client =
            OpenAiModelClient::new("test-key", "wire-model").with_model_options(BTreeMap::from([
                ("reasoningEffort".to_string(), json!("high")),
                ("max_tokens".to_string(), json!(999999)),
            ]));
        let request = make_request(&std::env::temp_dir());

        let body = build_provider_request(
            ProviderRequestConfig {
                endpoint: client.endpoint,
                api_base: &client.api_base,
                api_key: &client.api_key,
                model: &client.model,
                max_tokens: client.max_tokens,
                temperature: client.temperature,
                model_options: &client.model_options,
                system_prompt_override: &client.system_prompt_override,
            },
            &request,
        )
        .unwrap()
        .body;

        assert_eq!(body["reasoningEffort"], json!("high"));
        assert_eq!(body["max_tokens"], json!(4096));
    }
}
