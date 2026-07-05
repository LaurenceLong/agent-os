use agent_os_sys::{ModelCapabilities, ModelLimit};
use serde::Deserialize;
use std::sync::OnceLock;

const BUILTIN_MODEL_CATALOG: &str = include_str!("model_catalog/defaults.json");

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalogEntry {
    pub name: String,
    pub limit: ModelLimit,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCatalogFile {
    fallback: ModelCatalogDefaults,
    models: Vec<ModelSpecFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCatalogDefaults {
    limit: ModelLimit,
    capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelSpecFile {
    providers: Vec<String>,
    name: String,
    limit: ModelLimit,
    capabilities: ModelCapabilities,
}

#[derive(Debug)]
struct ModelCatalog {
    fallback: ModelCatalogDefaults,
    models: Vec<ModelSpec>,
}

#[derive(Debug)]
struct ModelSpec {
    providers: Vec<String>,
    name: String,
    normalized_name: String,
    limit: ModelLimit,
    capabilities: ModelCapabilities,
}

pub(crate) fn default_model_config(provider_id: &str, request_name: &str) -> ModelCatalogEntry {
    known_model_config(provider_id, request_name)
        .unwrap_or_else(|| fallback_model_config(request_name))
}

pub(crate) fn known_model_config(
    provider_id: &str,
    request_name: &str,
) -> Option<ModelCatalogEntry> {
    let catalog = catalog();
    let provider = provider_id.trim().to_ascii_lowercase();
    let request_name = request_name.trim().to_ascii_lowercase();
    if request_name.is_empty() {
        return None;
    }

    find_model(catalog, &provider, &request_name, MatchScope::ProviderExact)
        .or_else(|| find_model(catalog, &provider, &request_name, MatchScope::AnyExact))
        .or_else(|| {
            let suffix = request_name.rsplit('/').next().unwrap_or(&request_name);
            find_model(catalog, &provider, suffix, MatchScope::ProviderSuffix)
                .or_else(|| find_model(catalog, &provider, suffix, MatchScope::AnySuffix))
        })
        .map(model_entry)
}

enum MatchScope {
    ProviderExact,
    AnyExact,
    ProviderSuffix,
    AnySuffix,
}

fn catalog() -> &'static ModelCatalog {
    static CATALOG: OnceLock<ModelCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| load_catalog_with_fallback(None))
}

fn load_catalog_with_fallback(preferred_catalog: Option<&str>) -> ModelCatalog {
    preferred_catalog
        .and_then(|content| parse_catalog(content).ok())
        .or_else(|| parse_catalog(BUILTIN_MODEL_CATALOG).ok())
        .unwrap_or_else(minimal_catalog)
}

fn parse_catalog(content: &str) -> Result<ModelCatalog, serde_json::Error> {
    let parsed: ModelCatalogFile = serde_json::from_str(content)?;
    Ok(ModelCatalog {
        fallback: parsed.fallback,
        models: parsed
            .models
            .into_iter()
            .map(|model| ModelSpec {
                providers: model
                    .providers
                    .into_iter()
                    .map(|provider| provider.trim().to_ascii_lowercase())
                    .collect(),
                normalized_name: model.name.trim().to_ascii_lowercase(),
                name: model.name,
                limit: model.limit,
                capabilities: model.capabilities,
            })
            .collect(),
    })
}

fn minimal_catalog() -> ModelCatalog {
    ModelCatalog {
        fallback: ModelCatalogDefaults {
            limit: ModelLimit {
                context: 128_000,
                input: None,
                output: 16_000,
            },
            capabilities: ModelCapabilities {
                streaming: true,
                tool_calling: true,
                reasoning: true,
                temperature: true,
                image_input: false,
                structured_output: true,
            },
        },
        models: Vec::new(),
    }
}

fn find_model<'a>(
    catalog: &'a ModelCatalog,
    provider: &str,
    value: &str,
    scope: MatchScope,
) -> Option<&'a ModelSpec> {
    catalog.models.iter().find(|spec| {
        let provider_matches = spec.providers.iter().any(|known| known == provider);
        match scope {
            MatchScope::ProviderExact => provider_matches && spec.normalized_name == value,
            MatchScope::AnyExact => spec.normalized_name == value,
            MatchScope::ProviderSuffix => provider_matches && spec.normalized_name.ends_with(value),
            MatchScope::AnySuffix => spec.normalized_name.ends_with(value),
        }
    })
}

fn model_entry(spec: &ModelSpec) -> ModelCatalogEntry {
    ModelCatalogEntry {
        name: spec.name.clone(),
        limit: spec.limit.clone(),
        capabilities: spec.capabilities.clone(),
    }
}

fn fallback_model_config(request_name: &str) -> ModelCatalogEntry {
    let catalog = catalog();
    ModelCatalogEntry {
        name: request_name.trim().to_string(),
        limit: catalog.fallback.limit.clone(),
        capabilities: catalog.fallback.capabilities.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_preferred_catalog_falls_back_to_builtin_catalog() {
        let catalog = load_catalog_with_fallback(Some("{not json"));

        assert!(catalog
            .models
            .iter()
            .any(|model| model.normalized_name == "gpt-4o"));
    }

    #[test]
    fn invalid_catalog_content_has_minimal_fallback_contract() {
        let catalog = parse_catalog("{not json").unwrap_or_else(|_| minimal_catalog());

        assert_eq!(catalog.fallback.limit.context, 128_000);
        assert_eq!(catalog.fallback.limit.output, 16_000);
        assert!(catalog.fallback.capabilities.streaming);
        assert!(catalog.fallback.capabilities.tool_calling);
        assert!(catalog.fallback.capabilities.reasoning);
        assert!(catalog.fallback.capabilities.temperature);
        assert!(!catalog.fallback.capabilities.image_input);
        assert!(catalog.fallback.capabilities.structured_output);
    }
}
