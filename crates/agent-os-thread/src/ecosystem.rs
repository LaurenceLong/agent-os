mod mcp;
mod scan;

use agent_os_kernel::Kernel;
use agent_os_sys::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct EcosystemImportReport {
    pub instructions: usize,
    pub skills: usize,
    pub commands: usize,
    pub mcp_servers: usize,
    pub mcp_tools: usize,
    pub agents: usize,
}

pub fn import_workspace_ecosystem(
    kernel: &Kernel,
    workspace_root: &Path,
) -> AgentOsResult<EcosystemImportReport> {
    let root = workspace_root
        .canonicalize()
        .map_err(|error| AgentOsError::Validation(format!("canonicalize workspace: {error}")))?;
    let config_path = root.join("agent-os.json");
    let config = scan::read_agent_os_config(&config_path)?;
    let mut report = EcosystemImportReport::default();

    for document in scan::discover_instruction_documents(&root, &config)? {
        kernel.import_instruction_document(document)?;
        report.instructions += 1;
    }
    for skill in scan::discover_skills(&root)? {
        kernel.import_skill_definition(skill)?;
        report.skills += 1;
    }
    for command in scan::discover_commands(&root)? {
        kernel.import_command_definition(command)?;
        report.commands += 1;
    }
    for profile in scan::discover_agent_profiles(&root)? {
        kernel.register_imported_agent_profile(profile)?;
        report.agents += 1;
    }
    for (server, tools) in mcp::discover_mcp(&config_path, &config)? {
        kernel.register_mcp_server_spec(server)?;
        report.mcp_servers += 1;
        for tool in tools {
            kernel.register_mcp_tool_definition(tool)?;
            report.mcp_tools += 1;
        }
    }
    Ok(report)
}

pub fn expand_command_template(template: &str, args: &[String], raw_arguments: &str) -> String {
    let mut expanded = template.replace("$ARGUMENTS", raw_arguments);
    for index in 1..=9 {
        let token = format!("${index}");
        let value = args.get(index - 1).map(String::as_str).unwrap_or("");
        expanded = expanded.replace(&token, value);
    }
    expanded
}

pub(super) fn hash_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn stable_id(prefix: &str, path: &Path, salt: &str) -> String {
    let hash = hash_text(&format!("{}\n{salt}", path.to_string_lossy()));
    format!("{prefix}_{}", &hash[..16])
}

pub(super) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}
