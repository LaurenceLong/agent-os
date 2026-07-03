use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn install_package_manifest(
        &self,
        manifest: PackageManifestRecord,
        provenance: PackageInstallProvenance,
    ) -> AgentOsResult<PackageInstallRecord> {
        validate_package_manifest_record(&manifest)?;
        if provenance.installed_by.trim().is_empty() {
            return Err(AgentOsError::Validation(
                "package install provenance installed_by must not be empty".to_string(),
            ));
        }
        if self
            .read_state()?
            .package_installs
            .contains_key(&manifest.manifest.package_name)
        {
            return Err(AgentOsError::Validation(format!(
                "package {} is already installed",
                manifest.manifest.package_name
            )));
        }
        let now = now_rfc3339();
        let install = PackageInstallRecord {
            package_install_id: new_id("pkg_install_"),
            package_id: manifest.package_id,
            manifest: manifest.manifest,
            root_path: manifest.root_path,
            manifest_path: manifest.manifest_path,
            source: manifest.source,
            content_hash: manifest.content_hash,
            status: PackageInstallStatus::Enabled,
            disabled_reason: None,
            trust_policy: PackageTrustPolicy {
                require_signature: false,
                signature_verified: false,
            },
            install_provenance: provenance,
            cache_path: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.emit(
            "PackageInstalled",
            "package_install",
            &install.package_install_id,
            None,
            None,
            None,
            None,
            &install,
        )?;
        Ok(install)
    }

    pub fn enable_package(&self, package_name: &str) -> AgentOsResult<PackageInstallRecord> {
        self.set_package_status(package_name, PackageInstallStatus::Enabled, None)
    }

    pub fn disable_package(
        &self,
        package_name: &str,
        reason: impl Into<String>,
    ) -> AgentOsResult<PackageInstallRecord> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(AgentOsError::Validation(
                "package disable reason must not be empty".to_string(),
            ));
        }
        self.set_package_status(package_name, PackageInstallStatus::Disabled, Some(reason))
    }

    fn set_package_status(
        &self,
        package_name: &str,
        status: PackageInstallStatus,
        disabled_reason: Option<String>,
    ) -> AgentOsResult<PackageInstallRecord> {
        let mut install = self
            .read_state()?
            .package_installs
            .get(package_name)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("package {package_name}")))?;
        install.status = status;
        install.disabled_reason = disabled_reason;
        install.updated_at = now_rfc3339();
        let event_type = match status {
            PackageInstallStatus::Enabled => "PackageEnabled",
            PackageInstallStatus::Disabled => "PackageDisabled",
        };
        self.emit(
            event_type,
            "package_install",
            &install.package_install_id,
            None,
            None,
            None,
            None,
            &install,
        )?;
        Ok(install)
    }
}

fn validate_package_manifest_record(record: &PackageManifestRecord) -> AgentOsResult<()> {
    for (field, value) in [
        ("package_id", record.package_id.as_str()),
        ("package_name", record.manifest.package_name.as_str()),
        ("version", record.manifest.version.as_str()),
        ("root_path", record.root_path.as_str()),
        ("manifest_path", record.manifest_path.as_str()),
        ("source_path", record.source.source_path.as_str()),
        ("content_hash", record.content_hash.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(AgentOsError::Validation(format!(
                "package manifest record field {field} must not be empty"
            )));
        }
    }
    Ok(())
}
