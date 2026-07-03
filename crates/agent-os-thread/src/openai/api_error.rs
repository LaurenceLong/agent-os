use agent_os_sys::AgentOsError;
use serde_json::{json, Value};

const ERROR_BODY_LIMIT: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderApiErrorKind {
    ContextOverflow,
    Authentication,
    Authorization,
    RateLimited,
    Quota,
    InvalidRequest,
    ModelNotFound,
    Transient,
    Unknown,
}

impl ProviderApiErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ContextOverflow => "context_overflow",
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::RateLimited => "rate_limited",
            Self::Quota => "quota",
            Self::InvalidRequest => "invalid_request",
            Self::ModelNotFound => "model_not_found",
            Self::Transient => "transient",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderApiError {
    provider: String,
    status_code: Option<u16>,
    kind: ProviderApiErrorKind,
    retryable: bool,
    retry_after_ms: Option<u64>,
    message: String,
    response_body: Option<String>,
}

impl ProviderApiError {
    pub(crate) fn from_status(
        provider: &str,
        status_code: u16,
        response_body: String,
        retry_after_ms: Option<&str>,
        retry_after: Option<&str>,
    ) -> Self {
        let parsed_body = parse_json_body(&response_body);
        let message = extract_message(parsed_body.as_ref())
            .unwrap_or_else(|| fallback_status_message(status_code, &response_body));
        let kind = classify(status_code, &message, parsed_body.as_ref());
        let retryable = retryable(kind, status_code);
        Self {
            provider: provider.to_string(),
            status_code: Some(status_code),
            kind,
            retryable,
            retry_after_ms: parse_retry_after_ms(retry_after_ms, retry_after),
            message,
            response_body: (!response_body.is_empty()).then(|| truncate(&response_body)),
        }
    }

    pub(crate) fn from_transport(provider: &str, error: &ureq::Error) -> Self {
        let message = error.to_string();
        let lower = message.to_ascii_lowercase();
        let retryable = lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("connection")
            || lower.contains("dns")
            || lower.contains("temporarily");
        Self {
            provider: provider.to_string(),
            status_code: None,
            kind: if retryable {
                ProviderApiErrorKind::Transient
            } else {
                ProviderApiErrorKind::Unknown
            },
            retryable,
            retry_after_ms: None,
            message,
            response_body: None,
        }
    }

    pub(crate) fn to_audit_event(&self) -> Value {
        json!({
            "type": "provider_error",
            "provider": self.provider,
            "kind": self.kind.as_str(),
            "status_code": self.status_code,
            "retryable": self.retryable,
            "retry_after_ms": self.retry_after_ms,
            "message": self.message,
            "response_body": self.response_body,
        })
    }

    pub(crate) fn into_agent_error(self) -> AgentOsError {
        let status = self
            .status_code
            .map(|code| format!(" HTTP {code}"))
            .unwrap_or_default();
        let retry = if self.retryable {
            match self.retry_after_ms {
                Some(ms) => format!(" retryable=true retry_after_ms={ms}"),
                None => " retryable=true".to_string(),
            }
        } else {
            " retryable=false".to_string()
        };
        let message = format!(
            "{} API error kind={}{}{}: {}",
            self.provider,
            self.kind.as_str(),
            status,
            retry,
            self.message
        );
        match self.kind {
            ProviderApiErrorKind::ContextOverflow | ProviderApiErrorKind::Quota => {
                AgentOsError::BudgetExhausted(message)
            }
            ProviderApiErrorKind::Authentication | ProviderApiErrorKind::Authorization => {
                AgentOsError::PermissionDenied(message)
            }
            ProviderApiErrorKind::RateLimited => AgentOsError::ResourceConflict(message),
            ProviderApiErrorKind::InvalidRequest
            | ProviderApiErrorKind::ModelNotFound
            | ProviderApiErrorKind::Transient
            | ProviderApiErrorKind::Unknown => AgentOsError::Validation(message),
        }
    }
}

fn parse_json_body(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

fn extract_message(body: Option<&Value>) -> Option<String> {
    let body = body?;
    for pointer in [
        "/error/message",
        "/message",
        "/error",
        "/detail",
        "/error_description",
    ] {
        if let Some(message) = body.pointer(pointer).and_then(Value::as_str) {
            if !message.trim().is_empty() {
                return Some(message.trim().to_string());
            }
        }
    }
    None
}

fn fallback_status_message(status_code: u16, response_body: &str) -> String {
    if response_body.trim().is_empty() {
        return format!("provider returned HTTP {status_code}");
    }
    let trimmed = response_body.trim();
    if trimmed.to_ascii_lowercase().starts_with("<!doctype html")
        || trimmed.to_ascii_lowercase().starts_with("<html")
    {
        return format!("provider returned HTML error body for HTTP {status_code}");
    }
    truncate(trimmed)
}

fn classify(status_code: u16, message: &str, parsed_body: Option<&Value>) -> ProviderApiErrorKind {
    let lower = message.to_ascii_lowercase();
    let code = parsed_body
        .and_then(|body| {
            body.pointer("/error/code")
                .or_else(|| body.pointer("/code"))
        })
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let error_type = parsed_body
        .and_then(|body| {
            body.pointer("/error/type")
                .or_else(|| body.pointer("/type"))
        })
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    if status_code == 413
        || code.contains("context_length")
        || code.contains("context_window")
        || lower.contains("prompt is too long")
        || lower.contains("input is too long")
        || lower.contains("context window")
        || lower.contains("context length")
        || lower.contains("maximum context")
        || lower.contains("maximum prompt length")
        || lower.contains("reduce the length of the messages")
        || lower.contains("request entity too large")
        || lower.contains("token limit")
    {
        return ProviderApiErrorKind::ContextOverflow;
    }
    if status_code == 401 {
        return ProviderApiErrorKind::Authentication;
    }
    if status_code == 403 {
        return ProviderApiErrorKind::Authorization;
    }
    if code.contains("insufficient_quota")
        || code.contains("usage_not_included")
        || lower.contains("quota")
        || lower.contains("billing")
        || lower.contains("usage limit")
    {
        return ProviderApiErrorKind::Quota;
    }
    if status_code == 429 || error_type.contains("rate_limit") || code.contains("rate_limit") {
        return ProviderApiErrorKind::RateLimited;
    }
    if status_code == 404 || code.contains("model_not_found") || lower.contains("model not found") {
        return ProviderApiErrorKind::ModelNotFound;
    }
    if status_code == 400 || status_code == 422 || code.contains("invalid") {
        return ProviderApiErrorKind::InvalidRequest;
    }
    if status_code == 408 || status_code == 409 || status_code >= 500 {
        return ProviderApiErrorKind::Transient;
    }
    ProviderApiErrorKind::Unknown
}

fn retryable(kind: ProviderApiErrorKind, status_code: u16) -> bool {
    match kind {
        ProviderApiErrorKind::ContextOverflow
        | ProviderApiErrorKind::Authentication
        | ProviderApiErrorKind::Authorization
        | ProviderApiErrorKind::Quota
        | ProviderApiErrorKind::InvalidRequest
        | ProviderApiErrorKind::ModelNotFound => false,
        ProviderApiErrorKind::RateLimited | ProviderApiErrorKind::Transient => true,
        ProviderApiErrorKind::Unknown => {
            status_code == 408 || status_code == 409 || status_code >= 500
        }
    }
}

fn parse_retry_after_ms(retry_after_ms: Option<&str>, retry_after: Option<&str>) -> Option<u64> {
    if let Some(value) = retry_after_ms.and_then(parse_positive_u64) {
        return Some(value);
    }
    retry_after
        .and_then(parse_positive_u64)
        .map(|seconds| seconds.saturating_mul(1000))
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    let parsed = value.trim().parse::<u64>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn truncate(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars().take(ERROR_BODY_LIMIT) {
        out.push(ch);
    }
    if value.chars().count() > ERROR_BODY_LIMIT {
        out.push_str("...[truncated]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_context_overflow_without_retry() {
        let error = ProviderApiError::from_status(
            "openai-compatible",
            400,
            json!({
                "error": {
                    "code": "context_length_exceeded",
                    "message": "maximum context length is 128000 tokens"
                }
            })
            .to_string(),
            None,
            None,
        );

        assert_eq!(error.kind, ProviderApiErrorKind::ContextOverflow);
        assert!(!error.retryable);
        assert!(matches!(
            error.into_agent_error(),
            AgentOsError::BudgetExhausted(message)
                if message.contains("context_overflow")
        ));
    }

    #[test]
    fn classifies_rate_limit_with_retry_after_header() {
        let error = ProviderApiError::from_status(
            "openai-compatible",
            429,
            json!({"error": {"message": "rate limit reached"}}).to_string(),
            Some("1500"),
            None,
        );

        assert_eq!(error.kind, ProviderApiErrorKind::RateLimited);
        assert!(error.retryable);
        assert_eq!(error.retry_after_ms, Some(1500));
    }

    #[test]
    fn classifies_quota_as_budget_exhausted() {
        let error = ProviderApiError::from_status(
            "openai-compatible",
            429,
            json!({
                "error": {
                    "code": "insufficient_quota",
                    "message": "quota exceeded"
                }
            })
            .to_string(),
            None,
            Some("2"),
        );

        assert_eq!(error.kind, ProviderApiErrorKind::Quota);
        assert!(!error.retryable);
        assert_eq!(error.retry_after_ms, Some(2000));
        assert!(matches!(
            error.into_agent_error(),
            AgentOsError::BudgetExhausted(message) if message.contains("quota")
        ));
    }
}
