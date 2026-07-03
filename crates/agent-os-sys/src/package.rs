use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageType {
    Agent,
    PolicyPack,
    Distro,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSignature {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub manifest_version: String,
    pub package_name: String,
    pub package_type: PackageType,
    pub version: String,
    pub entrypoint: String,
    pub required_kernel_version: String,
    pub capabilities_requested: Vec<String>,
    pub roles_provided: Vec<String>,
    pub tools_provided: Vec<String>,
    pub schemas: Vec<String>,
    pub signature: Option<PackageSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifestRecord {
    pub package_id: String,
    pub manifest: PackageManifest,
    pub root_path: String,
    pub manifest_path: String,
    pub source: crate::EcosystemSource,
    pub content_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageInstallStatus {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageTrustPolicy {
    pub require_signature: bool,
    pub signature_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInstallProvenance {
    pub installed_by: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInstallRecord {
    pub package_install_id: String,
    pub package_id: String,
    pub manifest: PackageManifest,
    pub root_path: String,
    pub manifest_path: String,
    pub source: crate::EcosystemSource,
    pub content_hash: String,
    pub status: PackageInstallStatus,
    pub disabled_reason: Option<String>,
    pub trust_policy: PackageTrustPolicy,
    pub install_provenance: PackageInstallProvenance,
    pub cache_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageContributionKind {
    InstructionDocument,
    SkillDefinition,
    CommandDefinition,
    McpServer,
    McpTool,
    ImportedAgentProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageContributionRecord {
    pub package_contribution_id: String,
    pub package_install_id: String,
    pub package_id: String,
    pub package_name: String,
    pub contribution_kind: PackageContributionKind,
    pub contribution_id: String,
    pub contribution_name: String,
    pub source: crate::EcosystemSource,
    pub content_hash: Option<String>,
    pub created_at: String,
}
