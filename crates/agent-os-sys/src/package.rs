use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageType {
    Agent,
    PolicyPack,
    Distro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSignature {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
