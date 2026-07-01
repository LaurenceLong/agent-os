use agent_os_sys::{AgentOsError, AgentOsResult};
use agent_os_thread::LlmApiStyle;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GlobalProviderConfig {
    default_provider: String,
    providers: BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    api_key: String,
    base_url: String,
    model: String,
    api_style: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedProvider {
    pub(crate) name: String,
    pub(crate) model: String,
}

impl GlobalProviderConfig {
    pub(crate) fn load() -> AgentOsResult<Self> {
        let path = global_provider_config_path()?;
        let content = fs::read_to_string(&path).map_err(|error| {
            AgentOsError::Validation(format!(
                "read global provider config {}: {error}",
                path.display()
            ))
        })?;
        Self::parse(&path, &content)
    }

    fn parse(path: &std::path::Path, content: &str) -> AgentOsResult<Self> {
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let config: Self = serde_json::from_str(content).map_err(|error| {
            AgentOsError::Validation(format!(
                "parse global provider config {}: {error}",
                path.display()
            ))
        })?;
        config.validate(path)?;
        Ok(config)
    }

    pub(crate) fn resolve(&self, provider_name: Option<&str>) -> AgentOsResult<ResolvedProvider> {
        let name = provider_name.unwrap_or(&self.default_provider);
        let provider = self.providers.get(name).ok_or_else(|| {
            AgentOsError::Validation(format!("global provider config has no provider `{name}`"))
        })?;
        Ok(ResolvedProvider {
            name: name.to_string(),
            model: provider.model.clone(),
        })
    }

    fn validate(&self, path: &std::path::Path) -> AgentOsResult<()> {
        if self.default_provider.trim().is_empty() {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} must set default_provider",
                path.display()
            )));
        }
        if self.providers.is_empty() {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} must define at least one provider",
                path.display()
            )));
        }
        for (name, provider) in &self.providers {
            if name.trim().is_empty()
                || provider.api_key.trim().is_empty()
                || provider.base_url.trim().is_empty()
                || provider.model.trim().is_empty()
                || provider.api_style.trim().is_empty()
            {
                return Err(AgentOsError::Validation(format!(
                    "global provider config {} has an incomplete provider entry",
                    path.display()
                )));
            }
            LlmApiStyle::from_value(&provider.api_style)?;
        }
        if !self.providers.contains_key(&self.default_provider) {
            return Err(AgentOsError::Validation(format!(
                "global provider config {} default_provider `{}` is not defined",
                path.display(),
                self.default_provider
            )));
        }
        Ok(())
    }
}

pub(crate) fn global_provider_config_path() -> AgentOsResult<PathBuf> {
    let config_dir = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                AgentOsError::Validation(
                    "APPDATA is required to locate global provider config".to_string(),
                )
            })?
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .ok_or_else(|| {
                AgentOsError::Validation(
                    "XDG_CONFIG_HOME is required to locate global provider config".to_string(),
                )
            })?
    };
    Ok(config_dir.join("agent-os").join("providers.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn global_provider_config_resolves_default_provider() {
        let config = GlobalProviderConfig {
            default_provider: "default".to_string(),
            providers: BTreeMap::from([(
                "default".to_string(),
                ProviderEntry {
                    api_key: "test-key".to_string(),
                    base_url: "https://api.example.test/v1".to_string(),
                    model: "test-model".to_string(),
                    api_style: "openai-compatible".to_string(),
                },
            )]),
        };
        let resolved = config.resolve(None).unwrap();
        assert_eq!(resolved.name, "default");
        assert_eq!(resolved.model, "test-model");
    }

    #[test]
    fn global_provider_config_loads_utf8_bom_json() {
        let config = GlobalProviderConfig::parse(
            Path::new("providers.json"),
            "\u{feff}{\"default_provider\":\"default\",\"providers\":{\"default\":{\"api_key\":\"test-key\",\"base_url\":\"https://api.example.test/v1\",\"model\":\"test-model\",\"api_style\":\"openai-compatible\"}}}",
        )
        .unwrap();
        let resolved = config.resolve(None).unwrap();

        assert_eq!(resolved.name, "default");
        assert_eq!(resolved.model, "test-model");
    }
}
