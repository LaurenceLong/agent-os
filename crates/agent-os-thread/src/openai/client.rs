use super::audit::{append_jsonl, truncate};
use super::messages::{build_anthropic_messages, build_messages};
use super::parser::{parse_anthropic_response, parse_response};
use super::prompt::default_system_prompt;
use super::tools::{anthropic_tool_definitions, tool_definitions};
use crate::{ModelClient, ModelTurnRequest, ModelTurnResponse};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::path::PathBuf;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MAX_TOKENS: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmApiStyle {
    OpenAiCompatible,
    AnthropicCompatible,
}

impl LlmApiStyle {
    pub fn from_value(value: &str) -> AgentOsResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" | "openai-compatible" | "openai_compatible" | "chat-completions"
            | "chat_completions" => Ok(Self::OpenAiCompatible),
            "anthropic" | "anthropic-compatible" | "anthropic_compatible" | "messages" => {
                Ok(Self::AnthropicCompatible)
            }
            other => Err(AgentOsError::Validation(format!(
                "unsupported LLM_API_STYLE {other}; expected openai-compatible or anthropic-compatible"
            ))),
        }
    }

    pub fn from_env_or_base(api_base: &str) -> AgentOsResult<Self> {
        if let Ok(style) =
            std::env::var("LLM_API_STYLE").or_else(|_| std::env::var("AGENT_OS_API_STYLE"))
        {
            return Self::from_value(&style);
        }
        Ok(Self::from_base(api_base))
    }

    fn from_base(api_base: &str) -> Self {
        if api_base
            .trim_end_matches('/')
            .to_ascii_lowercase()
            .ends_with("/anthropic")
        {
            return Self::AnthropicCompatible;
        }
        Self::OpenAiCompatible
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiModelClient {
    api_base: String,
    api_key: String,
    model: String,
    max_tokens: u64,
    temperature: f64,
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
            temperature: 0.0,
            system_prompt_override: None,
            api_style: LlmApiStyle::OpenAiCompatible,
            audit_log_path: None,
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self.api_style = LlmApiStyle::from_base(&self.api_base);
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
        self.temperature = temperature;
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

    pub fn from_env() -> AgentOsResult<Self> {
        let api_key = std::env::var("LLM_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .map_err(|_| {
                AgentOsError::Validation(
                "LLM_API_KEY or OPENAI_API_KEY environment variable is required for model client"
                    .to_string(),
            )
            })?;
        let model = std::env::var("LLM_MODEL")
            .or_else(|_| std::env::var("AGENT_OS_MODEL"))
            .unwrap_or_else(|_| "gpt-4o".to_string());
        let api_base = std::env::var("LLM_BASE_URL")
            .or_else(|_| std::env::var("OPENAI_API_BASE"))
            .unwrap_or_else(|_| DEFAULT_API_BASE.to_string());
        let api_style = LlmApiStyle::from_env_or_base(&api_base)?;
        Ok(Self::new(api_key, model)
            .with_api_base(api_base)
            .with_api_style(api_style))
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
        let tools = tool_definitions();

        let body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
        });

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let bearer = format!("Bearer {}", self.api_key);
        self.audit_provider_event(json!({
            "type": "provider_request",
            "provider": "openai-compatible",
            "endpoint": "/chat/completions",
            "body": body.clone(),
        }))?;

        let response_body = match ureq::post(&url)
            .set("Authorization", &bearer)
            .set("Content-Type", "application/json")
            .send_json(&body)
        {
            Ok(response) => response.into_json::<Value>().map_err(|e| {
                AgentOsError::Validation(format!("failed to parse API response: {e}"))
            })?,
            Err(ureq::Error::Status(code, response)) => {
                let error_text = response.into_string().unwrap_or_default();
                let truncated = truncate(&error_text, 2048);
                return Err(AgentOsError::Validation(format!(
                    "OpenAI API error (HTTP {code}): {truncated}"
                )));
            }
            Err(e) => {
                return Err(AgentOsError::Validation(format!(
                    "OpenAI API request failed: {e}"
                )));
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
        let body = json!({
            "model": self.model,
            "system": self.system_prompt_override
                .clone()
                .unwrap_or_else(|| default_system_prompt(request, &workspace_root)),
            "messages": build_anthropic_messages(request, &workspace_root),
            "tools": anthropic_tool_definitions(),
            "tool_choice": {"type": "auto"},
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
        });

        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));
        let bearer = format!("Bearer {}", self.api_key);
        self.audit_provider_event(json!({
            "type": "provider_request",
            "provider": "anthropic-compatible",
            "endpoint": "/v1/messages",
            "body": body.clone(),
        }))?;
        let response_body = match ureq::post(&url)
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
                let error_text = response.into_string().unwrap_or_default();
                let truncated = truncate(&error_text, 2048);
                return Err(AgentOsError::Validation(format!(
                    "Anthropic-compatible API error (HTTP {code}): {truncated}"
                )));
            }
            Err(e) => {
                return Err(AgentOsError::Validation(format!(
                    "Anthropic-compatible API request failed: {e}"
                )));
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
}
