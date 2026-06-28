use agent_os_sys::{AgentOsError, AgentOsResult, PackageManifest, PackageType};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) struct SoftwareEngineeringDistro {
    pub(super) manifest: PackageManifest,
    pub(super) supervisor_prompt: String,
    pub(super) worker_prompt: String,
    pub(super) reviewer_prompt: String,
    pub(super) review_policy: Value,
    pub(super) final_answer_policy: Value,
}

impl SoftwareEngineeringDistro {
    pub(super) fn load_default() -> AgentOsResult<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("distros/software-engineering");
        Self::load(&root)
    }

    pub(super) fn load(root: &Path) -> AgentOsResult<Self> {
        let manifest: PackageManifest = serde_json::from_str(
            &fs::read_to_string(root.join("manifest.json")).map_err(|error| {
                AgentOsError::Validation(format!(
                    "failed to read software distro manifest: {error}"
                ))
            })?,
        )?;
        if manifest.package_type != PackageType::Distro {
            return Err(AgentOsError::Validation(
                "software-engineering package must be a distro".to_string(),
            ));
        }
        Ok(Self {
            manifest,
            supervisor_prompt: fs::read_to_string(root.join("prompts/supervisor.md")).map_err(
                |error| {
                    AgentOsError::Validation(format!("failed to read supervisor prompt: {error}"))
                },
            )?,
            worker_prompt: fs::read_to_string(root.join("prompts/worker.md")).map_err(|error| {
                AgentOsError::Validation(format!("failed to read worker prompt: {error}"))
            })?,
            reviewer_prompt: fs::read_to_string(root.join("prompts/reviewer.md")).map_err(
                |error| {
                    AgentOsError::Validation(format!("failed to read reviewer prompt: {error}"))
                },
            )?,
            review_policy: serde_json::from_str(
                &fs::read_to_string(root.join("policy/review.json")).map_err(|error| {
                    AgentOsError::Validation(format!("failed to read review policy: {error}"))
                })?,
            )?,
            final_answer_policy: serde_json::from_str(
                &fs::read_to_string(root.join("policy/final-answer.json")).map_err(|error| {
                    AgentOsError::Validation(format!("failed to read final-answer policy: {error}"))
                })?,
            )?,
        })
    }
}
