//! Agent-OS configuration, path, and provider resolution.
//!
//! This crate is the single boundary for host/CLI-visible filesystem policy:
//! global config, project overrides, and global runtime data paths.

mod model_catalog;
mod provider;

pub use agent_os_sys::{ModelCapabilities, ModelLimit};
pub use provider::{
    AgentOsConfigFile, ModelCapabilitiesConfig, ModelConfigEntry, ModelEntry, ModelRef,
    ProviderCatalog, ProviderConfigEntry, ProviderEntry, ProviderOptions, ProviderOptionsEntry,
    ResolvedModel,
};

use agent_os_sys::{AgentOsError, AgentOsResult};
use provider::{
    merge_config, read_config_file, read_required_config_file, reject_project_provider_authority,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const AGENT_OS_HOME_ENV: &str = "AGENT_OS_HOME";
pub const CONFIG_FILE_NAME: &str = "config.json";
pub const CONFIG_BACKUP_DIR_NAME: &str = "backup";
pub const CONFIG_BACKUP_FILE_NAME: &str = "config.last-good.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOsPaths {
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
    pub bin_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIdentity {
    pub canonical_root: PathBuf,
    pub slug: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimePaths {
    pub project: ProjectIdentity,
    pub project_data_dir: PathBuf,
    pub state_db: PathBuf,
    pub artifact_blobs: PathBuf,
    pub evidence_blobs: PathBuf,
    pub provider_audit_log: PathBuf,
    pub runtime_log_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentOsConfig {
    pub paths: AgentOsPaths,
    pub project: Option<ProjectIdentity>,
    pub providers: ProviderCatalog,
    pub global_config_recovery: Option<GlobalConfigRecovery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfigRecovery {
    pub primary_path: PathBuf,
    pub backup_path: PathBuf,
    pub primary_error: String,
}

impl AgentOsPaths {
    pub fn resolve() -> AgentOsResult<Self> {
        Self::resolve_with_env(|key| std::env::var_os(key).map(PathBuf::from))
    }

    fn resolve_with_env(mut env: impl FnMut(&str) -> Option<PathBuf>) -> AgentOsResult<Self> {
        if let Some(home) = env(AGENT_OS_HOME_ENV) {
            return Ok(Self {
                config_dir: home.join("config"),
                data_dir: home.join("data"),
                state_dir: home.join("state"),
                cache_dir: home.join("cache"),
                log_dir: home.join("log"),
                bin_dir: home.join("cache").join("bin"),
                home,
            });
        }

        if cfg!(windows) {
            let appdata = env("APPDATA").ok_or_else(|| {
                AgentOsError::Validation(
                    "APPDATA is required to locate Agent-OS config".to_string(),
                )
            })?;
            let local = env("LOCALAPPDATA").ok_or_else(|| {
                AgentOsError::Validation(
                    "LOCALAPPDATA is required to locate Agent-OS runtime data".to_string(),
                )
            })?;
            let config_dir = appdata.join("agent-os");
            let data_dir = local.join("agent-os");
            return Ok(Self {
                home: data_dir.clone(),
                config_dir,
                state_dir: data_dir.join("state"),
                cache_dir: data_dir.join("cache"),
                log_dir: data_dir.join("log"),
                bin_dir: data_dir.join("cache").join("bin"),
                data_dir,
            });
        }

        let home = env("HOME").ok_or_else(|| {
            AgentOsError::Validation("HOME is required to locate Agent-OS paths".to_string())
        })?;
        let config_dir = env("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home.join(".config"))
            .join("agent-os");
        let data_dir = env("XDG_DATA_HOME")
            .unwrap_or_else(|| home.join(".local").join("share"))
            .join("agent-os");
        let state_dir = env("XDG_STATE_HOME")
            .unwrap_or_else(|| home.join(".local").join("state"))
            .join("agent-os");
        let cache_dir = env("XDG_CACHE_HOME")
            .unwrap_or_else(|| home.join(".cache"))
            .join("agent-os");
        let log_dir = state_dir.join("log");
        Ok(Self {
            home: data_dir.clone(),
            config_dir,
            data_dir,
            state_dir,
            cache_dir: cache_dir.clone(),
            log_dir,
            bin_dir: cache_dir.join("bin"),
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE_NAME)
    }

    pub fn config_backup_file(&self) -> PathBuf {
        self.config_dir
            .join(CONFIG_BACKUP_DIR_NAME)
            .join(CONFIG_BACKUP_FILE_NAME)
    }

    pub fn default_state_db(&self) -> PathBuf {
        self.state_dir.join("agent-os.sqlite")
    }

    pub fn project_runtime_paths(
        &self,
        workspace: impl AsRef<Path>,
    ) -> AgentOsResult<ProjectRuntimePaths> {
        let project = ProjectIdentity::from_workspace(workspace)?;
        let project_data_dir = self.data_dir.join("projects").join(format!(
            "{}-{}",
            project.slug,
            &project.hash[..16]
        ));
        Ok(ProjectRuntimePaths {
            project,
            state_db: self.default_state_db(),
            artifact_blobs: project_data_dir.join("blobs").join("artifacts"),
            evidence_blobs: project_data_dir.join("blobs").join("evidence"),
            provider_audit_log: self.log_dir.join("provider-audit.jsonl"),
            runtime_log_dir: project_data_dir.join("runtime"),
            project_data_dir,
        })
    }

    pub fn create_runtime_dirs(&self, runtime: &ProjectRuntimePaths) -> AgentOsResult<()> {
        for path in [
            &self.config_dir,
            &self.data_dir,
            &self.state_dir,
            &self.cache_dir,
            &self.log_dir,
            &self.bin_dir,
            &runtime.project_data_dir,
            &runtime.artifact_blobs,
            &runtime.evidence_blobs,
            &runtime.runtime_log_dir,
        ] {
            fs::create_dir_all(path).map_err(|error| {
                AgentOsError::Validation(format!(
                    "create Agent-OS directory {}: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(())
    }
}

impl ProjectIdentity {
    pub fn from_workspace(workspace: impl AsRef<Path>) -> AgentOsResult<Self> {
        let canonical_root = workspace.as_ref().canonicalize().map_err(|error| {
            AgentOsError::Validation(format!(
                "canonicalize workspace {}: {error}",
                workspace.as_ref().display()
            ))
        })?;
        let slug = canonical_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(sanitize_slug)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "workspace".to_string());
        let hash = sha256_hex(&canonical_root.to_string_lossy());
        Ok(Self {
            canonical_root,
            slug,
            hash,
        })
    }
}

impl ResolvedAgentOsConfig {
    pub fn load(workspace: Option<&Path>) -> AgentOsResult<Self> {
        let paths = AgentOsPaths::resolve()?;
        Self::load_with_paths(paths, workspace)
    }

    pub fn load_with_paths(paths: AgentOsPaths, workspace: Option<&Path>) -> AgentOsResult<Self> {
        let project_config = load_project_config(workspace)?;
        let primary_path = paths.config_file();
        let backup_path = paths.config_backup_file();
        let primary = read_required_config_file(&primary_path).and_then(|global| {
            resolve_provider_catalog(global.clone(), project_config.clone())
                .map(|providers| (global, providers))
        });
        let (providers, global_config_recovery) = match primary {
            Ok((_, providers)) => {
                let _ = refresh_global_config_backup(&primary_path, &backup_path);
                (providers, None)
            }
            Err(primary_error) => {
                let primary_error = primary_error.to_string();
                let backup = read_required_config_file(&backup_path)
                    .and_then(|global| resolve_provider_catalog(global, project_config.clone()));
                let providers = backup.map_err(|backup_error| {
                    AgentOsError::Validation(format!(
                        "Agent-OS global config {} failed ({primary_error}); backup {} failed ({backup_error})",
                        primary_path.display(),
                        backup_path.display()
                    ))
                })?;
                (
                    providers,
                    Some(GlobalConfigRecovery {
                        primary_path: primary_path.clone(),
                        backup_path,
                        primary_error,
                    }),
                )
            }
        };
        let project = workspace.map(ProjectIdentity::from_workspace).transpose()?;
        Ok(Self {
            paths,
            project,
            providers,
            global_config_recovery,
        })
    }
}

fn load_project_config(workspace: Option<&Path>) -> AgentOsResult<AgentOsConfigFile> {
    let Some(workspace) = workspace else {
        return Ok(AgentOsConfigFile::default());
    };
    let project_path = workspace.join(".agent-os").join(CONFIG_FILE_NAME);
    let project = read_config_file(&project_path)?;
    reject_project_provider_authority(&project_path, &project)?;
    Ok(project)
}

fn resolve_provider_catalog(
    global_config: AgentOsConfigFile,
    project_config: AgentOsConfigFile,
) -> AgentOsResult<ProviderCatalog> {
    let mut merged = AgentOsConfigFile::default();
    merge_config(&mut merged, global_config);
    merge_config(&mut merged, project_config);
    ProviderCatalog::from_config(merged)
}

fn refresh_global_config_backup(primary_path: &Path, backup_path: &Path) -> AgentOsResult<()> {
    let content = fs::read(primary_path).map_err(|error| {
        AgentOsError::Validation(format!(
            "read Agent-OS primary config {} for backup: {error}",
            primary_path.display()
        ))
    })?;
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AgentOsError::Validation(format!(
                "create Agent-OS config backup directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let tmp_path = backup_path.with_extension("json.tmp");
    fs::write(&tmp_path, content).map_err(|error| {
        AgentOsError::Validation(format!(
            "write Agent-OS config backup {}: {error}",
            tmp_path.display()
        ))
    })?;
    if backup_path.exists() {
        fs::remove_file(backup_path).map_err(|error| {
            AgentOsError::Validation(format!(
                "replace Agent-OS config backup {}: {error}",
                backup_path.display()
            ))
        })?;
    }
    fs::rename(&tmp_path, backup_path).map_err(|error| {
        AgentOsError::Validation(format!(
            "publish Agent-OS config backup {}: {error}",
            backup_path.display()
        ))
    })
}

fn sanitize_slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' {
            if !last_dash && !out.is_empty() {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(next);
            last_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn agent_os_home_provides_isolated_cross_platform_roots() {
        let root = PathBuf::from("/tmp/agent-os-test-home");
        let paths =
            AgentOsPaths::resolve_with_env(|key| (key == AGENT_OS_HOME_ENV).then(|| root.clone()))
                .unwrap();

        assert_eq!(paths.config_file(), root.join("config").join("config.json"));
        assert_eq!(
            paths.config_backup_file(),
            root.join("config")
                .join("backup")
                .join("config.last-good.json")
        );
        assert_eq!(
            paths.default_state_db(),
            root.join("state").join("agent-os.sqlite")
        );
        assert_eq!(paths.bin_dir, root.join("cache").join("bin"));
    }

    #[test]
    fn unix_paths_use_xdg_with_home_fallbacks() {
        if cfg!(windows) {
            return;
        }
        let paths = AgentOsPaths::resolve_with_env(|key| match key {
            "HOME" => Some(PathBuf::from("/home/tester")),
            "XDG_CONFIG_HOME" => Some(PathBuf::from("/xdg/config")),
            "XDG_DATA_HOME" => Some(PathBuf::from("/xdg/data")),
            "XDG_STATE_HOME" => Some(PathBuf::from("/xdg/state")),
            "XDG_CACHE_HOME" => Some(PathBuf::from("/xdg/cache")),
            _ => None,
        })
        .unwrap();

        assert_eq!(paths.config_dir, PathBuf::from("/xdg/config/agent-os"));
        assert_eq!(paths.data_dir, PathBuf::from("/xdg/data/agent-os"));
        assert_eq!(paths.state_dir, PathBuf::from("/xdg/state/agent-os"));
        assert_eq!(paths.cache_dir, PathBuf::from("/xdg/cache/agent-os"));
    }

    #[test]
    fn project_runtime_paths_use_global_state_db_and_project_blobs() {
        let root = temp_dir("agent-os-config-runtime");
        fs::create_dir_all(&root).unwrap();
        let paths = AgentOsPaths::resolve_with_env(|key| {
            (key == AGENT_OS_HOME_ENV).then(|| root.join("home"))
        })
        .unwrap();

        let runtime = paths.project_runtime_paths(&root).unwrap();

        assert_eq!(runtime.state_db, paths.default_state_db());
        assert!(runtime
            .project_data_dir
            .starts_with(paths.data_dir.join("projects")));
        assert!(runtime.artifact_blobs.ends_with("blobs/artifacts"));
        assert!(runtime.evidence_blobs.ends_with("blobs/evidence"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn successful_global_config_refreshes_last_good_backup() {
        let (root, paths) = test_paths("agent-os-config-backup-refresh");
        write_global_config(&paths, &valid_global_config("openai/gpt-4o"));

        let config = ResolvedAgentOsConfig::load_with_paths(paths.clone(), None).unwrap();

        assert!(config.global_config_recovery.is_none());
        assert_eq!(
            paths.config_backup_file(),
            paths
                .config_dir
                .join("backup")
                .join("config.last-good.json")
        );
        let backup: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.config_backup_file()).unwrap()).unwrap();
        assert_eq!(backup["model"], "openai/gpt-4o");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_global_config_recovers_from_last_good_backup() {
        let (root, paths) = test_paths("agent-os-config-backup-malformed");
        write_global_config(&paths, &valid_global_config("openai/gpt-4o"));
        ResolvedAgentOsConfig::load_with_paths(paths.clone(), None).unwrap();
        write_global_config(&paths, "{");

        let config = ResolvedAgentOsConfig::load_with_paths(paths.clone(), None).unwrap();
        let recovery = config.global_config_recovery.unwrap();
        let model = config.providers.resolve(None).unwrap();

        assert_eq!(model.id, "openai/gpt-4o");
        assert_eq!(recovery.primary_path, paths.config_file());
        assert_eq!(recovery.backup_path, paths.config_backup_file());
        assert!(recovery.primary_error.contains("parse Agent-OS config"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_global_catalog_recovers_from_last_good_backup() {
        let (root, paths) = test_paths("agent-os-config-backup-invalid-catalog");
        write_global_config(&paths, &valid_global_config("openai/gpt-4o"));
        ResolvedAgentOsConfig::load_with_paths(paths.clone(), None).unwrap();
        write_global_config(&paths, &valid_global_config("openai/missing-model"));

        let config = ResolvedAgentOsConfig::load_with_paths(paths.clone(), None).unwrap();
        let recovery = config.global_config_recovery.unwrap();
        let model = config.providers.resolve(None).unwrap();

        assert_eq!(model.id, "openai/gpt-4o");
        assert!(recovery.primary_error.contains("has no model"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_project_config_is_not_masked_by_global_backup() {
        let (root, paths) = test_paths("agent-os-config-backup-project-invalid");
        let workspace = root.join("workspace");
        fs::create_dir_all(workspace.join(".agent-os")).unwrap();
        write_global_config(&paths, &valid_global_config("openai/gpt-4o"));
        ResolvedAgentOsConfig::load_with_paths(paths.clone(), Some(&workspace)).unwrap();
        fs::write(
            workspace.join(".agent-os").join(CONFIG_FILE_NAME),
            r#"{"provider":{"openai":{"api_key":"project-secret"}}}"#,
        )
        .unwrap();

        let error = ResolvedAgentOsConfig::load_with_paths(paths, Some(&workspace)).unwrap_err();

        assert!(error
            .to_string()
            .contains("must not contain provider api_key"));
        let _ = fs::remove_dir_all(root);
    }

    fn test_paths(prefix: &str) -> (PathBuf, AgentOsPaths) {
        let root = temp_dir(prefix);
        fs::create_dir_all(&root).unwrap();
        let home = root.join("home");
        let paths =
            AgentOsPaths::resolve_with_env(|key| (key == AGENT_OS_HOME_ENV).then(|| home.clone()))
                .unwrap();
        (root, paths)
    }

    fn write_global_config(paths: &AgentOsPaths, content: &str) {
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.config_file(), content).unwrap();
    }

    fn valid_global_config(default_model: &str) -> String {
        serde_json::json!({
            "model": default_model,
            "provider": {
                "openai": {
                    "api_key": "test-key",
                    "api_style": "openai-compatible",
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
        .to_string()
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
