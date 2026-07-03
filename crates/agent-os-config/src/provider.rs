use crate::model_catalog;
use agent_os_sys::{AgentOsError, AgentOsResult, LlmApiStyle, ModelCapabilities, ModelLimit};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AgentOsConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provider: BTreeMap<String, ProviderConfigEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfigEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "ProviderOptions::is_empty")]
    pub options: ProviderOptions,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ModelConfigEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

impl ProviderOptions {
    pub fn is_empty(&self) -> bool {
        self.base_url.is_none() && self.timeout_ms.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModelConfigEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<ModelLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilitiesConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilitiesConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calling: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<bool>,
}

impl ModelCapabilitiesConfig {
    pub fn from_capabilities(capabilities: ModelCapabilities) -> Self {
        Self {
            streaming: Some(capabilities.streaming),
            tool_calling: Some(capabilities.tool_calling),
            reasoning: Some(capabilities.reasoning),
            temperature: Some(capabilities.temperature),
            image_input: Some(capabilities.image_input),
            structured_output: Some(capabilities.structured_output),
        }
    }

    fn apply_to(&self, capabilities: &mut ModelCapabilities) {
        if let Some(value) = self.streaming {
            capabilities.streaming = value;
        }
        if let Some(value) = self.tool_calling {
            capabilities.tool_calling = value;
        }
        if let Some(value) = self.reasoning {
            capabilities.reasoning = value;
        }
        if let Some(value) = self.temperature {
            capabilities.temperature = value;
        }
        if let Some(value) = self.image_input {
            capabilities.image_input = value;
        }
        if let Some(value) = self.structured_output {
            capabilities.structured_output = value;
        }
    }

    fn merge(&mut self, next: Self) {
        if next.streaming.is_some() {
            self.streaming = next.streaming;
        }
        if next.tool_calling.is_some() {
            self.tool_calling = next.tool_calling;
        }
        if next.reasoning.is_some() {
            self.reasoning = next.reasoning;
        }
        if next.temperature.is_some() {
            self.temperature = next.temperature;
        }
        if next.image_input.is_some() {
            self.image_input = next.image_input;
        }
        if next.structured_output.is_some() {
            self.structured_output = next.structured_output;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCatalog {
    pub model: String,
    pub small_model: Option<String>,
    pub provider: BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    pub api_key: String,
    pub endpoint: LlmApiStyle,
    pub options: ProviderOptionsEntry,
    pub models: BTreeMap<String, ModelEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOptionsEntry {
    pub base_url: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    pub name: String,
    pub options: BTreeMap<String, Value>,
    pub limit: ModelLimit,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModel {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub name: String,
    pub api_key: String,
    pub base_url: String,
    pub endpoint: LlmApiStyle,
    pub timeout_ms: Option<u64>,
    pub options: BTreeMap<String, Value>,
    pub limit: ModelLimit,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

impl ModelRef {
    pub fn parse(value: &str) -> AgentOsResult<Self> {
        if value.matches('/').count() != 1 {
            return Err(AgentOsError::Validation(format!(
                "Agent-OS model id `{value}` must have provider/model form"
            )));
        }
        let (provider_id, model_id) = value.split_once('/').ok_or_else(|| {
            AgentOsError::Validation(format!(
                "Agent-OS model id `{value}` must have provider/model form"
            ))
        })?;
        let provider_id = valid_id_segment(provider_id, "provider")?;
        let model_id = valid_id_segment(model_id, "model")?;
        Ok(Self {
            provider_id,
            model_id,
        })
    }

    pub fn full_id(&self) -> String {
        format!("{}/{}", self.provider_id, self.model_id)
    }
}

impl ProviderCatalog {
    pub fn load_default() -> AgentOsResult<Self> {
        let paths = crate::AgentOsPaths::resolve()?;
        Self::load_from_path(paths.config_file())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> AgentOsResult<Self> {
        Self::from_config(read_required_config_file(path.as_ref())?)
    }

    pub fn from_config(config: AgentOsConfigFile) -> AgentOsResult<Self> {
        let model = required_root_model(config.model)?;
        let model_ref = ModelRef::parse(&model)?;
        let small_model = config
            .small_model
            .map(|value| {
                let model_ref = ModelRef::parse(&value)?;
                Ok::<String, AgentOsError>(model_ref.full_id())
            })
            .transpose()?;
        if config.provider.is_empty() {
            return Err(AgentOsError::Validation(
                "Agent-OS config must define at least one provider".to_string(),
            ));
        }
        let provider = resolve_provider_entries(config.provider)?;
        let catalog = Self {
            model: model_ref.full_id(),
            small_model,
            provider,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn resolve(&self, model: Option<&str>) -> AgentOsResult<ResolvedModel> {
        let model_ref = ModelRef::parse(model.unwrap_or(&self.model))?;
        let provider = self.provider.get(&model_ref.provider_id).ok_or_else(|| {
            AgentOsError::Validation(format!(
                "Agent-OS config has no provider `{}`",
                model_ref.provider_id
            ))
        })?;
        let model = provider.models.get(&model_ref.model_id).ok_or_else(|| {
            AgentOsError::Validation(format!(
                "Agent-OS provider `{}` has no model `{}`",
                model_ref.provider_id, model_ref.model_id
            ))
        })?;
        Ok(ResolvedModel {
            id: model_ref.full_id(),
            provider_id: model_ref.provider_id,
            model_id: model_ref.model_id,
            name: model.name.clone(),
            api_key: provider.api_key.clone(),
            base_url: provider.options.base_url.clone(),
            endpoint: provider.endpoint,
            timeout_ms: provider.options.timeout_ms,
            options: model.options.clone(),
            limit: model.limit.clone(),
            capabilities: model.capabilities.clone(),
        })
    }

    fn validate(&self) -> AgentOsResult<()> {
        self.resolve(Some(&self.model))?;
        if let Some(small_model) = &self.small_model {
            self.resolve(Some(small_model))?;
        }
        Ok(())
    }
}

pub(crate) fn read_config_file(path: &Path) -> AgentOsResult<AgentOsConfigFile> {
    if !path.is_file() {
        return Ok(AgentOsConfigFile::default());
    }
    read_required_config_file(path)
}

pub(crate) fn read_required_config_file(path: &Path) -> AgentOsResult<AgentOsConfigFile> {
    let content = fs::read_to_string(path).map_err(|error| {
        AgentOsError::Validation(format!("read Agent-OS config {}: {error}", path.display()))
    })?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
    serde_json::from_str(content).map_err(|error| {
        AgentOsError::Validation(format!("parse Agent-OS config {}: {error}", path.display()))
    })
}

pub(crate) fn merge_config(base: &mut AgentOsConfigFile, next: AgentOsConfigFile) {
    if next.model.is_some() {
        base.model = next.model;
    }
    if next.small_model.is_some() {
        base.small_model = next.small_model;
    }
    for (provider_id, provider) in next.provider {
        base.provider
            .entry(provider_id)
            .and_modify(|base_provider| merge_provider(base_provider, provider.clone()))
            .or_insert(provider);
    }
}

pub(crate) fn reject_project_provider_authority(
    path: &Path,
    config: &AgentOsConfigFile,
) -> AgentOsResult<()> {
    if config.provider.values().any(|provider| {
        provider
            .api_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        return Err(AgentOsError::Validation(format!(
            "project Agent-OS config {} must not contain provider api_key values",
            path.display()
        )));
    }
    if config.provider.values().any(|provider| {
        provider
            .endpoint
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        return Err(AgentOsError::Validation(format!(
            "project Agent-OS config {} must not contain provider endpoint values",
            path.display()
        )));
    }
    Ok(())
}

fn merge_provider(base: &mut ProviderConfigEntry, next: ProviderConfigEntry) {
    if next.api_key.is_some() {
        base.api_key = next.api_key;
    }
    if next.endpoint.is_some() {
        base.endpoint = next.endpoint;
    }
    if next.options.base_url.is_some() {
        base.options.base_url = next.options.base_url;
    }
    if next.options.timeout_ms.is_some() {
        base.options.timeout_ms = next.options.timeout_ms;
    }
    for (model_id, model) in next.models {
        base.models
            .entry(model_id)
            .and_modify(|base_model| merge_model(base_model, model.clone()))
            .or_insert(model);
    }
}

fn merge_model(base: &mut ModelConfigEntry, next: ModelConfigEntry) {
    if next.name.is_some() {
        base.name = next.name;
    }
    if !next.options.is_empty() {
        base.options = next.options;
    }
    if next.limit.is_some() {
        base.limit = next.limit;
    }
    if let Some(next_capabilities) = next.capabilities {
        match base.capabilities.as_mut() {
            Some(base_capabilities) => base_capabilities.merge(next_capabilities),
            None => base.capabilities = Some(next_capabilities),
        }
    }
}

fn resolve_provider_entries(
    entries: BTreeMap<String, ProviderConfigEntry>,
) -> AgentOsResult<BTreeMap<String, ProviderEntry>> {
    entries
        .into_iter()
        .map(|(provider_id, provider)| {
            let provider_id = valid_id_segment(&provider_id, "provider")?;
            if provider.models.is_empty() {
                return Err(AgentOsError::Validation(format!(
                    "Agent-OS config provider `{provider_id}` must define at least one model"
                )));
            }
            let models = provider
                .models
                .into_iter()
                .map(|(model_id, model)| {
                    let model_id = valid_id_segment(&model_id, "model")?;
                    let name = model.name.ok_or_else(|| {
                        AgentOsError::Validation(format!(
                            "Agent-OS config provider `{provider_id}` model `{model_id}` must define name"
                        ))
                    })?;
                    if name.trim().is_empty() {
                        return Err(AgentOsError::Validation(format!(
                            "Agent-OS config provider `{provider_id}` model `{model_id}` has empty name"
                        )));
                    }
                    let name = name.trim().to_string();
                    let limit = model.limit.ok_or_else(|| {
                        AgentOsError::Validation(format!(
                            "Agent-OS config provider `{provider_id}` model `{model_id}` must define limit.context and limit.output"
                        ))
                    })?;
                    limit.validate_for_model(&provider_id, &model_id)?;
                    let mut capabilities = model_catalog::default_model_capabilities(
                        &provider_id,
                        &model_id,
                    )
                    .or_else(|| {
                        model_catalog::default_model_capabilities(&provider_id, &name)
                    })
                    .unwrap_or_default();
                    if let Some(configured_capabilities) = &model.capabilities {
                        configured_capabilities.apply_to(&mut capabilities);
                    }
                    Ok((
                        model_id,
                        ModelEntry {
                            name,
                            options: model.options,
                            limit,
                            capabilities,
                        },
                    ))
                })
                .collect::<AgentOsResult<BTreeMap<_, _>>>()?;
            let endpoint = LlmApiStyle::from_value(&required_provider_field(
                &provider_id,
                provider.endpoint,
                "endpoint",
            )?)?;
            Ok((
                provider_id.clone(),
                ProviderEntry {
                    api_key: required_provider_field(&provider_id, provider.api_key, "api_key")?,
                    endpoint,
                    options: ProviderOptionsEntry {
                        base_url: required_provider_field(
                            &provider_id,
                            provider.options.base_url,
                            "options.base_url",
                        )?,
                        timeout_ms: provider.options.timeout_ms,
                    },
                    models,
                },
            ))
        })
        .collect()
}

fn required_root_model(value: Option<String>) -> AgentOsResult<String> {
    let value = value
        .ok_or_else(|| AgentOsError::Validation("Agent-OS config must set model".to_string()))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(AgentOsError::Validation(
            "Agent-OS config model must not be empty".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn required_provider_field(
    provider_id: &str,
    value: Option<String>,
    field: &str,
) -> AgentOsResult<String> {
    let value = value.ok_or_else(|| {
        AgentOsError::Validation(format!(
            "Agent-OS config provider `{provider_id}` is missing `{field}`"
        ))
    })?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AgentOsError::Validation(format!(
            "Agent-OS config provider `{provider_id}` has empty `{field}`"
        )));
    }
    Ok(trimmed.to_string())
}

fn valid_id_segment(value: &str, kind: &str) -> AgentOsResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.chars().any(char::is_whitespace) {
        return Err(AgentOsError::Validation(format!(
            "Agent-OS {kind} id `{value}` must be non-empty and must not contain `/` or whitespace"
        )));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentOsPaths, ResolvedAgentOsConfig, AGENT_OS_HOME_ENV};
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn global_config_resolves_provider_model_id() {
        let catalog = ProviderCatalog::from_config(AgentOsConfigFile {
            model: Some("openai/gpt-4o".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("global-key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: Some(120000),
                    },
                    models: BTreeMap::from([(
                        "gpt-4o".to_string(),
                        ModelConfigEntry {
                            name: Some("gpt-4o".to_string()),
                            options: BTreeMap::from([(
                                "reasoningEffort".to_string(),
                                json!("low"),
                            )]),
                            limit: Some(test_limit(128000, 16384)),
                            capabilities: Some(ModelCapabilitiesConfig {
                                tool_calling: Some(true),
                                reasoning: Some(true),
                                ..ModelCapabilitiesConfig::default()
                            }),
                        },
                    )]),
                },
            )]),
            small_model: None,
        })
        .unwrap();

        let model = catalog.resolve(None).unwrap();

        assert_eq!(model.id, "openai/gpt-4o");
        assert_eq!(model.provider_id, "openai");
        assert_eq!(model.model_id, "gpt-4o");
        assert_eq!(model.name, "gpt-4o");
        assert_eq!(model.api_key, "global-key");
        assert_eq!(model.base_url, "https://api.example.test/v1");
        assert_eq!(model.timeout_ms, Some(120000));
        assert_eq!(model.options["reasoningEffort"], "low");
        assert_eq!(model.limit.context, 128000);
        assert_eq!(model.limit.output, 16384);
        assert!(model.capabilities.tool_calling);
        assert!(model.capabilities.reasoning);
    }

    #[test]
    fn global_and_project_config_merge_with_project_model_selection() {
        let root = temp_dir("agent-os-config-merge");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join("config")).unwrap();
        fs::create_dir_all(workspace.join(".agent-os")).unwrap();
        fs::write(
            home.join("config/config.json"),
            json!({
                "model": "openai/gpt-4o",
                "small_model": "openai/gpt-4o-mini",
                "provider": {
                    "openai": {
                        "api_key": "global-key",
                        "endpoint": "openai_chat_completions",
                        "options": {
                            "base_url": "https://api.example.test/v1",
                            "timeout_ms": 120000
                        },
                            "models": {
                            "gpt-4o": {
                                "name": "gpt-4o",
                                "limit": {"context": 128000, "output": 16384}
                            },
                            "gpt-4o-mini": {
                                "name": "gpt-4o-mini",
                                "limit": {"context": 128000, "output": 16384}
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            workspace.join(".agent-os/config.json"),
            json!({
                "model": "openai/project-model",
                "provider": {
                    "openai": {
                        "options": {
                            "base_url": "https://proxy.example.test/v1"
                        },
                        "models": {
                            "project-model": {
                                "name": "provider-project-model",
                                "options": {"reasoningEffort": "high"},
                                "limit": {"context": 200000, "input": 180000, "output": 8192},
                                "capabilities": {"structured_output": true, "reasoning": true}
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let paths =
            AgentOsPaths::resolve_with_env(|key| (key == AGENT_OS_HOME_ENV).then(|| home.clone()))
                .unwrap();

        let config = ResolvedAgentOsConfig::load_with_paths(paths, Some(&workspace)).unwrap();
        let model = config.providers.resolve(None).unwrap();

        assert_eq!(model.id, "openai/project-model");
        assert_eq!(model.api_key, "global-key");
        assert_eq!(model.base_url, "https://proxy.example.test/v1");
        assert_eq!(model.name, "provider-project-model");
        assert_eq!(model.options["reasoningEffort"], "high");
        assert_eq!(model.limit.context, 200000);
        assert_eq!(model.limit.input, Some(180000));
        assert_eq!(model.limit.output, 8192);
        assert!(model.capabilities.structured_output);
        assert!(model.capabilities.reasoning);
        assert_eq!(
            config.providers.small_model.as_deref(),
            Some("openai/gpt-4o-mini")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_config_rejects_provider_api_keys() {
        let root = temp_dir("agent-os-config-project-secret");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join("config")).unwrap();
        fs::create_dir_all(workspace.join(".agent-os")).unwrap();
        fs::write(
            home.join("config/config.json"),
            json!({
                "model": "openai/gpt-4o",
                "provider": {
                    "openai": {
                        "api_key": "global-key",
                        "endpoint": "openai_chat_completions",
                        "options": {"base_url": "https://api.example.test/v1"},
                            "models": {
                            "gpt-4o": {
                                "name": "gpt-4o",
                                "limit": {"context": 128000, "output": 16384}
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            workspace.join(".agent-os/config.json"),
            json!({
                "provider": {
                    "openai": {
                        "api_key": "nope",
                        "models": {"local-model": {}}
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let paths =
            AgentOsPaths::resolve_with_env(|key| (key == AGENT_OS_HOME_ENV).then(|| home.clone()))
                .unwrap();

        let error = ResolvedAgentOsConfig::load_with_paths(paths, Some(&workspace)).unwrap_err();

        assert!(error
            .to_string()
            .contains("must not contain provider api_key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_model_ids_are_rejected() {
        for model in [
            "gpt-4o",
            "openai/org/gpt-4o",
            "/gpt-4o",
            "openai/",
            "open ai/gpt-4o",
            "openai/gpt 4o",
            "openai/gpt\t4o",
        ] {
            let error = ModelRef::parse(model).unwrap_err();
            assert!(
                error.to_string().contains("provider/model")
                    || error.to_string().contains("non-empty")
                    || error.to_string().contains("whitespace")
            );
        }
    }

    #[test]
    fn missing_provider_or_model_is_rejected() {
        let base = AgentOsConfigFile {
            model: Some("anthropic/sonnet".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([("gpt-4o".to_string(), test_model_config())]),
                },
            )]),
            small_model: None,
        };

        let error = ProviderCatalog::from_config(base).unwrap_err();
        assert!(error.to_string().contains("no provider `anthropic`"));

        let missing_model = AgentOsConfigFile {
            model: Some("openai/missing".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([("gpt-4o".to_string(), test_model_config())]),
                },
            )]),
            small_model: None,
        };

        let error = ProviderCatalog::from_config(missing_model).unwrap_err();
        assert!(error.to_string().contains("has no model `missing`"));
    }

    #[test]
    fn model_name_and_limit_are_required_and_validated() {
        let missing_name = AgentOsConfigFile {
            model: Some("openai/custom-model".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "custom-model".to_string(),
                        ModelConfigEntry {
                            limit: Some(test_limit(128000, 16384)),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        };
        let error = ProviderCatalog::from_config(missing_name).unwrap_err();
        assert!(error.to_string().contains("must define name"));

        let missing_limit = AgentOsConfigFile {
            model: Some("openai/custom-model".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "custom-model".to_string(),
                        ModelConfigEntry {
                            name: Some("provider-custom-model".to_string()),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        };
        let error = ProviderCatalog::from_config(missing_limit).unwrap_err();
        assert!(error.to_string().contains("must define limit.context"));

        let invalid_input = AgentOsConfigFile {
            model: Some("openai/custom-model".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "custom-model".to_string(),
                        ModelConfigEntry {
                            name: Some("provider-custom-model".to_string()),
                            limit: Some(ModelLimit {
                                context: 1000,
                                input: Some(1001),
                                output: 128,
                            }),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        };
        let error = ProviderCatalog::from_config(invalid_input).unwrap_err();
        assert!(error
            .to_string()
            .contains("limit.input must not exceed limit.context"));
    }

    #[test]
    fn known_model_catalog_supplies_default_image_capability() {
        let catalog = ProviderCatalog::from_config(AgentOsConfigFile {
            model: Some("tongyi/qwen3.6-plus".to_string()),
            provider: BTreeMap::from([(
                "tongyi".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "qwen3.6-plus".to_string(),
                        ModelConfigEntry {
                            name: Some("qwen3.6-plus".to_string()),
                            limit: Some(test_limit(128000, 16384)),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        })
        .unwrap();

        let model = catalog.resolve(None).unwrap();

        assert!(model.capabilities.image_input);
        assert!(model.capabilities.tool_calling);
    }

    #[test]
    fn known_configured_name_supplies_default_image_capability() {
        let catalog = ProviderCatalog::from_config(AgentOsConfigFile {
            model: Some("tongyi/primary".to_string()),
            provider: BTreeMap::from([(
                "tongyi".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "primary".to_string(),
                        ModelConfigEntry {
                            name: Some("qwen3.6-plus".to_string()),
                            limit: Some(test_limit(128000, 16384)),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        })
        .unwrap();

        let model = catalog.resolve(None).unwrap();

        assert_eq!(model.name, "qwen3.6-plus");
        assert!(model.capabilities.image_input);
    }

    #[test]
    fn omitted_model_capabilities_fail_closed_for_image_input() {
        let catalog = ProviderCatalog::from_config(AgentOsConfigFile {
            model: Some("openai/text-model".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "text-model".to_string(),
                        ModelConfigEntry {
                            name: Some("provider-text-model".to_string()),
                            limit: Some(test_limit(128000, 16384)),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        })
        .unwrap();

        let model = catalog.resolve(None).unwrap();

        assert!(!model.capabilities.image_input);
        assert!(model.capabilities.is_empty());
    }

    #[test]
    fn provider_json_false_only_capability_overrides_catalog_field() {
        let mut global = AgentOsConfigFile {
            model: Some("openai/gpt-4o".to_string()),
            provider: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfigEntry {
                    api_key: Some("key".to_string()),
                    endpoint: Some("openai_chat_completions".to_string()),
                    options: ProviderOptions {
                        base_url: Some("https://api.example.test/v1".to_string()),
                        timeout_ms: None,
                    },
                    models: BTreeMap::from([(
                        "gpt-4o".to_string(),
                        ModelConfigEntry {
                            name: Some("gpt-4o".to_string()),
                            limit: Some(test_limit(128000, 16384)),
                            ..ModelConfigEntry::default()
                        },
                    )]),
                },
            )]),
            small_model: None,
        };
        let project: AgentOsConfigFile = serde_json::from_value(json!({
            "provider": {
                "openai": {
                    "models": {
                        "gpt-4o": {
                            "capabilities": {
                                "image_input": false
                            }
                        }
                    }
                }
            }
        }))
        .unwrap();

        merge_config(&mut global, project);
        let catalog = ProviderCatalog::from_config(global).unwrap();
        let model = catalog.resolve(None).unwrap();

        assert!(model.capabilities.tool_calling);
        assert!(!model.capabilities.image_input);
        assert!(model.capabilities.structured_output);
    }

    #[test]
    fn distros_provider_example_uses_explicit_model_names() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../distros/providers.example.json");
        let config = read_required_config_file(&path).unwrap();

        for (provider_id, provider) in &config.provider {
            for (model_id, model) in &provider.models {
                assert!(
                    model
                        .name
                        .as_deref()
                        .is_some_and(|name| !name.trim().is_empty()),
                    "{provider_id}/{model_id} must define explicit provider request name"
                );
            }
        }

        ProviderCatalog::from_config(config).unwrap();
    }

    fn test_model_config() -> ModelConfigEntry {
        ModelConfigEntry {
            name: Some("gpt-4o".to_string()),
            limit: Some(test_limit(128000, 16384)),
            capabilities: Some(ModelCapabilitiesConfig {
                tool_calling: Some(true),
                ..ModelCapabilitiesConfig::default()
            }),
            ..ModelConfigEntry::default()
        }
    }

    fn test_limit(context: u64, output: u64) -> ModelLimit {
        ModelLimit {
            context,
            input: None,
            output,
        }
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
