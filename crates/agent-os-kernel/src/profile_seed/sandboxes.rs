use agent_os_sys::*;

pub(super) fn core_sandboxes(now: &str) -> Vec<SandboxProfile> {
    vec![
        SandboxProfile {
            sandbox_profile_id: "sbox_readonly".to_string(),
            status: ProfileStatus::Active,
            name: "ReadOnlyLocal".to_string(),
            filesystem_mode: FilesystemMode::ReadOnly,
            network_mode: NetworkMode::Off,
            process_backend: ProcessBackend::Native,
            secret_policy: SecretPolicy::None,
            toolchain_profile_id: None,
            mount_policy: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            superseded_by: None,
        },
        SandboxProfile {
            sandbox_profile_id: "sbox_workspace_write".to_string(),
            status: ProfileStatus::Active,
            name: "WorkspaceWrite".to_string(),
            filesystem_mode: FilesystemMode::WorkspaceWrite,
            network_mode: NetworkMode::Allowlist,
            process_backend: ProcessBackend::Native,
            secret_policy: SecretPolicy::ScopedHandles,
            toolchain_profile_id: None,
            mount_policy: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            superseded_by: None,
        },
        SandboxProfile {
            sandbox_profile_id: "sbox_test_temp".to_string(),
            status: ProfileStatus::Active,
            name: "TestTempOutputs".to_string(),
            filesystem_mode: FilesystemMode::TempOnly,
            network_mode: NetworkMode::Off,
            process_backend: ProcessBackend::Native,
            secret_policy: SecretPolicy::None,
            toolchain_profile_id: None,
            mount_policy: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            superseded_by: None,
        },
    ]
}
