//! Agent-OS ecosystem discovery.
//!
//! This crate discovers global and project-local instructions, skills,
//! commands, agent profiles, and MCP servers. It does not own runtime behavior;
//! host code imports the resulting catalog into the kernel.

use agent_os_config::{AgentOsPaths, CONFIG_FILE_NAME};
use agent_os_kernel::discover_mcp_tool_definitions;
use agent_os_sys::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EcosystemImportReport {
    pub instructions: usize,
    pub skills: usize,
    pub commands: usize,
    pub mcp_servers: usize,
    pub mcp_tools: usize,
    pub agents: usize,
    pub sources: Vec<EcosystemSourceImportReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcosystemSourceImportReport {
    pub source_kind: EcosystemSourceKind,
    pub source_scope: EcosystemSourceScope,
    pub source_path: String,
    pub precedence_rank: Option<u32>,
    pub instructions: usize,
    pub skills: usize,
    pub commands: usize,
    pub mcp_servers: usize,
    pub mcp_tools: usize,
    pub agents: usize,
}

#[derive(Debug, Clone, Default)]
pub struct EcosystemCatalog {
    pub instruction_documents: Vec<InstructionDocument>,
    pub skill_definitions: Vec<SkillDefinition>,
    pub command_definitions: Vec<CommandDefinition>,
    pub mcp_servers: Vec<McpServerSpec>,
    pub mcp_tools: Vec<McpToolDefinition>,
    pub imported_agent_profiles: Vec<ImportedAgentProfile>,
}

impl EcosystemCatalog {
    pub fn import_report(&self) -> EcosystemImportReport {
        let mut report = EcosystemImportReport::default();
        for document in &self.instruction_documents {
            report.instructions += 1;
            source_report_mut(
                &mut report.sources,
                &document.source,
                Some(document.precedence_rank),
            )
            .instructions += 1;
        }
        for skill in &self.skill_definitions {
            report.skills += 1;
            source_report_mut(&mut report.sources, &skill.source, None).skills += 1;
        }
        for command in &self.command_definitions {
            report.commands += 1;
            source_report_mut(&mut report.sources, &command.source, None).commands += 1;
        }
        for server in &self.mcp_servers {
            report.mcp_servers += 1;
            source_report_mut(&mut report.sources, &server.source, None).mcp_servers += 1;
        }
        for tool in &self.mcp_tools {
            report.mcp_tools += 1;
            source_report_mut(&mut report.sources, &tool.source, None).mcp_tools += 1;
        }
        for profile in &self.imported_agent_profiles {
            report.agents += 1;
            source_report_mut(&mut report.sources, &profile.source, None).agents += 1;
        }
        report.sources.sort_by(|left, right| {
            left.precedence_rank
                .unwrap_or(u32::MAX)
                .cmp(&right.precedence_rank.unwrap_or(u32::MAX))
                .then_with(|| left.source_path.cmp(&right.source_path))
        });
        report
    }
}

#[derive(Debug, Clone)]
pub struct EcosystemDiscoverOptions {
    pub workspace_root: PathBuf,
    pub paths: AgentOsPaths,
}

impl EcosystemDiscoverOptions {
    pub fn for_workspace(workspace_root: impl Into<PathBuf>) -> AgentOsResult<Self> {
        Ok(Self {
            workspace_root: workspace_root.into(),
            paths: AgentOsPaths::resolve()?,
        })
    }
}

#[derive(Debug, Clone)]
struct EcosystemRoot {
    path: PathBuf,
    kind: EcosystemSourceKind,
    scope: EcosystemSourceScope,
    rank: u32,
}

impl EcosystemRoot {
    fn source(&self, path: &Path) -> EcosystemSource {
        EcosystemSource {
            source_kind: self.kind,
            source_scope: self.scope,
            source_path: path.to_string_lossy().to_string(),
        }
    }
}

pub fn discover_ecosystem(options: &EcosystemDiscoverOptions) -> AgentOsResult<EcosystemCatalog> {
    let workspace_root = options.workspace_root.canonicalize().map_err(|error| {
        AgentOsError::Validation(format!(
            "canonicalize workspace {}: {error}",
            options.workspace_root.display()
        ))
    })?;
    let mut roots = ecosystem_roots(&workspace_root, &options.paths);
    roots.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut instructions = discover_instruction_documents(&workspace_root, &options.paths, &roots)?;
    instructions.sort_by(|left, right| {
        left.precedence_rank
            .cmp(&right.precedence_rank)
            .then_with(|| left.source.source_path.cmp(&right.source.source_path))
            .then_with(|| left.instruction_id.cmp(&right.instruction_id))
    });

    let mut skills = BTreeMap::new();
    let mut commands = BTreeMap::new();
    let mut agents = BTreeMap::new();
    for root in &roots {
        for skill in discover_skills(root)? {
            skills.insert(skill.name.clone(), skill);
        }
        for command in discover_commands(root)? {
            commands.insert(command.name.clone(), command);
        }
        for profile in discover_agent_profiles(root)? {
            agents.insert(profile.name.clone(), profile);
        }
    }

    let mut mcp_servers = BTreeMap::new();
    let mut mcp_tools = BTreeMap::new();
    for (path, scope, rank) in config_files(&workspace_root, &options.paths) {
        let config = read_json_config(&path)?;
        for (server, tools) in discover_mcp(&path, scope, rank, &config)? {
            mcp_servers.insert(server.name.clone(), server);
            for tool in tools {
                mcp_tools.insert(tool.model_tool_name.clone(), tool);
            }
        }
    }

    Ok(EcosystemCatalog {
        instruction_documents: instructions,
        skill_definitions: skills.into_values().collect(),
        command_definitions: commands.into_values().collect(),
        mcp_servers: mcp_servers.into_values().collect(),
        mcp_tools: mcp_tools.into_values().collect(),
        imported_agent_profiles: agents.into_values().collect(),
    })
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

fn source_report_mut<'a>(
    sources: &'a mut Vec<EcosystemSourceImportReport>,
    source: &EcosystemSource,
    precedence_rank: Option<u32>,
) -> &'a mut EcosystemSourceImportReport {
    if let Some(index) = sources.iter().position(|candidate| {
        candidate.source_kind == source.source_kind
            && candidate.source_scope == source.source_scope
            && candidate.source_path == source.source_path
    }) {
        let report = &mut sources[index];
        match (report.precedence_rank, precedence_rank) {
            (Some(current), Some(next)) if next < current => {
                report.precedence_rank = Some(next);
            }
            (None, Some(next)) => {
                report.precedence_rank = Some(next);
            }
            _ => {}
        }
        return report;
    }
    sources.push(EcosystemSourceImportReport {
        source_kind: source.source_kind,
        source_scope: source.source_scope,
        source_path: source.source_path.clone(),
        precedence_rank,
        instructions: 0,
        skills: 0,
        commands: 0,
        mcp_servers: 0,
        mcp_tools: 0,
        agents: 0,
    });
    let index = sources.len() - 1;
    &mut sources[index]
}

fn ecosystem_roots(workspace_root: &Path, paths: &AgentOsPaths) -> Vec<EcosystemRoot> {
    let mut roots = Vec::new();
    let mut rank = 0u32;
    if let Some(home) = home_dir() {
        for (path, kind) in [
            (home.join(".claude"), EcosystemSourceKind::Claude),
            (home.join(".agents"), EcosystemSourceKind::Agents),
            (home.join(".agent-os"), EcosystemSourceKind::AgentOs),
        ] {
            roots.push(root(path, kind, EcosystemSourceScope::Global, rank));
            rank += 1;
        }
    }
    roots.push(root(
        paths.config_dir.clone(),
        EcosystemSourceKind::AgentOs,
        EcosystemSourceScope::Global,
        rank,
    ));
    rank += 1;

    for ancestor in project_dirs(workspace_root) {
        for (path, kind) in [
            (ancestor.join(".claude"), EcosystemSourceKind::Claude),
            (ancestor.join(".agents"), EcosystemSourceKind::Agents),
            (ancestor.join(".agent-os"), EcosystemSourceKind::AgentOs),
        ] {
            roots.push(root(path, kind, EcosystemSourceScope::Project, rank));
            rank += 1;
        }
    }
    roots
}

fn root(
    path: PathBuf,
    kind: EcosystemSourceKind,
    scope: EcosystemSourceScope,
    rank: u32,
) -> EcosystemRoot {
    EcosystemRoot {
        path,
        kind,
        scope,
        rank,
    }
}

fn discover_instruction_documents(
    workspace_root: &Path,
    paths: &AgentOsPaths,
    roots: &[EcosystemRoot],
) -> AgentOsResult<Vec<InstructionDocument>> {
    let mut documents = Vec::new();
    let mut rank = 0u32;
    for root in roots
        .iter()
        .filter(|root| root.scope == EcosystemSourceScope::Global)
    {
        for file in instruction_candidates_for_root(root) {
            if file.is_file() {
                documents.push(instruction_document(&file, root.source(&file), rank)?);
                rank += 1;
            }
        }
    }
    for dir in project_dirs(workspace_root) {
        for file in [dir.join("CLAUDE.md"), dir.join("AGENTS.md")] {
            if file.is_file() {
                documents.push(instruction_document(
                    &file,
                    EcosystemSource {
                        source_kind: source_kind_for_instruction(&file),
                        source_scope: EcosystemSourceScope::Project,
                        source_path: file.to_string_lossy().to_string(),
                    },
                    rank,
                )?);
                rank += 1;
            }
        }
    }
    for (config_path, scope, _) in config_files(workspace_root, paths) {
        let config = read_json_config(&config_path)?;
        if let Some(items) = config.get("instructions").and_then(Value::as_array) {
            for item in items {
                let raw = item.as_str().ok_or_else(|| {
                    AgentOsError::Validation(
                        "Agent-OS config instructions entries must be strings".to_string(),
                    )
                })?;
                if raw.starts_with("http://") || raw.starts_with("https://") || raw.contains('*') {
                    return Err(AgentOsError::Validation(
                        "Agent-OS config instructions support exact local paths only".to_string(),
                    ));
                }
                let path = if Path::new(raw).is_absolute() {
                    PathBuf::from(raw)
                } else if scope == EcosystemSourceScope::Global {
                    paths.config_dir.join(raw)
                } else {
                    workspace_root.join(raw)
                };
                if path.is_file() {
                    documents.push(instruction_document(
                        &path,
                        EcosystemSource {
                            source_kind: EcosystemSourceKind::AgentOs,
                            source_scope: EcosystemSourceScope::Config,
                            source_path: path.to_string_lossy().to_string(),
                        },
                        rank,
                    )?);
                    rank += 1;
                }
            }
        }
    }
    Ok(documents)
}

fn instruction_candidates_for_root(root: &EcosystemRoot) -> Vec<PathBuf> {
    match root.kind {
        EcosystemSourceKind::Claude => vec![root.path.join("CLAUDE.md")],
        EcosystemSourceKind::Agents => vec![root.path.join("AGENTS.md")],
        EcosystemSourceKind::AgentOs => {
            vec![root.path.join("AGENTS.md"), root.path.join("CLAUDE.md")]
        }
    }
}

fn discover_skills(root: &EcosystemRoot) -> AgentOsResult<Vec<SkillDefinition>> {
    let mut skills = Vec::new();
    for directory in ["skills", "skill"] {
        let skills_root = root.path.join(directory);
        if !skills_root.is_dir() {
            continue;
        }
        for skill_file in markdown_files_named(&skills_root, "SKILL.md")? {
            let skill_root = skill_file.parent().ok_or_else(|| {
                AgentOsError::Validation(format!(
                    "skill file has no parent: {}",
                    skill_file.display()
                ))
            })?;
            let (frontmatter, content) = parse_markdown_file(&skill_file)?;
            let name = optional_frontmatter(&frontmatter, "name").unwrap_or_else(|| {
                skill_root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });
            let description = optional_frontmatter(&frontmatter, "description")
                .or_else(|| markdown_description(&content))
                .ok_or_else(|| {
                    AgentOsError::Validation(format!(
                        "{} frontmatter missing description and no markdown summary was found",
                        skill_file.to_string_lossy()
                    ))
                })?;
            let hash = hash_text(&format!("{name}\n{description}\n{content}"));
            skills.push(SkillDefinition {
                skill_id: stable_id("skill", &skill_file, &hash),
                name,
                description,
                root_path: skill_root.to_string_lossy().to_string(),
                skill_file_path: skill_file.to_string_lossy().to_string(),
                source: root.source(&skill_file),
                content,
                metadata: BTreeMap::new(),
                content_hash: hash,
                created_at: now_rfc3339(),
            });
        }
    }
    Ok(skills)
}

fn discover_commands(root: &EcosystemRoot) -> AgentOsResult<Vec<CommandDefinition>> {
    let mut commands = Vec::new();
    for directory in ["commands", "command"] {
        let command_root = root.path.join(directory);
        if !command_root.is_dir() {
            continue;
        }
        for path in markdown_files(&command_root)? {
            let (frontmatter, template) = parse_markdown_file(&path)?;
            if template.contains("!`") {
                return Err(AgentOsError::Validation(format!(
                    "command {} uses unsupported shell interpolation",
                    path.to_string_lossy()
                )));
            }
            let rel = relative_without_extension(&command_root, &path)?;
            let hash = hash_text(&format!("{rel}\n{template}"));
            commands.push(CommandDefinition {
                command_id: stable_id("cmd", &path, &hash),
                name: rel,
                description: frontmatter.get("description").cloned(),
                agent: frontmatter.get("agent").cloned(),
                model: frontmatter.get("model").cloned(),
                argument_hints: command_hints(&template),
                template,
                source: root.source(&path),
                content_hash: hash,
                created_at: now_rfc3339(),
            });
        }
    }
    Ok(commands)
}

fn discover_agent_profiles(root: &EcosystemRoot) -> AgentOsResult<Vec<ImportedAgentProfile>> {
    let mut profiles = Vec::new();
    for directory in ["agents", "agent"] {
        let agent_root = root.path.join(directory);
        if !agent_root.is_dir() {
            continue;
        }
        for path in markdown_files(&agent_root)? {
            let (frontmatter, prompt) = parse_markdown_file(&path)?;
            let name = relative_without_extension(&agent_root, &path)?;
            let mode = match frontmatter.get("mode").map(String::as_str) {
                Some("primary") => ImportedAgentMode::Primary,
                Some("subagent") => ImportedAgentMode::Subagent,
                Some("all") | None => ImportedAgentMode::All,
                Some(other) => {
                    return Err(AgentOsError::Validation(format!(
                        "agent {} has invalid mode {other}",
                        path.to_string_lossy()
                    )))
                }
            };
            let hash = hash_text(&format!("{name}\n{prompt}"));
            profiles.push(ImportedAgentProfile {
                imported_agent_profile_id: stable_id("agent", &path, &hash),
                name,
                description: frontmatter.get("description").cloned(),
                mode,
                prompt,
                model: frontmatter.get("model").cloned(),
                role_profile_id: frontmatter.get("role_profile_id").cloned(),
                permission_profile_id: frontmatter.get("permission_profile_id").cloned(),
                source: root.source(&path),
                content_hash: hash,
                metadata: json!({}),
                created_at: now_rfc3339(),
            });
        }
    }
    Ok(profiles)
}

fn discover_mcp(
    config_path: &Path,
    scope: EcosystemSourceScope,
    rank: u32,
    config: &Value,
) -> AgentOsResult<Vec<(McpServerSpec, Vec<McpToolDefinition>)>> {
    let mut servers = Vec::new();
    let Some(local_stdio) = config
        .pointer("/mcp/local_stdio")
        .and_then(Value::as_object)
    else {
        return Ok(servers);
    };
    for (name, item) in local_stdio {
        let command = string_array_field(item, "command")?;
        let environment = string_map_field(item, "environment")?;
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let source = EcosystemSource {
            source_kind: EcosystemSourceKind::AgentOs,
            source_scope: scope,
            source_path: config_path.to_string_lossy().to_string(),
        };
        let server = McpServerSpec {
            server_id: stable_id("mcp", config_path, &format!("{rank}:{name}")),
            name: name.clone(),
            transport: McpTransportKind::LocalStdio,
            command,
            environment,
            enabled,
            timeout_ms: item
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(30_000),
            source: source.clone(),
            created_at: now_rfc3339(),
        };
        let tools = if enabled {
            discover_mcp_tool_definitions(&server, source)?
        } else {
            Vec::new()
        };
        servers.push((server, tools));
    }
    Ok(servers)
}

fn config_files(
    workspace_root: &Path,
    paths: &AgentOsPaths,
) -> Vec<(PathBuf, EcosystemSourceScope, u32)> {
    let mut files = Vec::new();
    let mut rank = 0u32;
    files.push((paths.config_file(), EcosystemSourceScope::Global, rank));
    rank += 1;
    for dir in project_dirs(workspace_root) {
        files.push((
            dir.join(".agent-os").join(CONFIG_FILE_NAME),
            EcosystemSourceScope::Project,
            rank,
        ));
        rank += 1;
    }
    files
}

fn project_dirs(workspace_root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = workspace_root.ancestors().map(Path::to_path_buf).collect();
    dirs.reverse();
    dirs
}

fn read_json_config(path: &Path) -> AgentOsResult<Value> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    serde_json::from_str(&fs::read_to_string(path).map_err(|error| {
        AgentOsError::Validation(format!(
            "read Agent-OS ecosystem config {}: {error}",
            path.display()
        ))
    })?)
    .map_err(AgentOsError::from)
}

fn instruction_document(
    path: &Path,
    source: EcosystemSource,
    precedence_rank: u32,
) -> AgentOsResult<InstructionDocument> {
    let content = fs::read_to_string(path)
        .map_err(|error| AgentOsError::Validation(format!("read instruction: {error}")))?;
    let hash = hash_text(&content);
    Ok(InstructionDocument {
        instruction_id: stable_id("inst", path, &hash),
        source,
        precedence_rank,
        content,
        content_hash: hash,
        created_at: now_rfc3339(),
    })
}

fn source_kind_for_instruction(path: &Path) -> EcosystemSourceKind {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(file) if file.eq_ignore_ascii_case("CLAUDE.md") => EcosystemSourceKind::Claude,
        _ => EcosystemSourceKind::Agents,
    }
}

fn parse_markdown_file(path: &Path) -> AgentOsResult<(BTreeMap<String, String>, String)> {
    let text = fs::read_to_string(path).map_err(|error| {
        AgentOsError::Validation(format!("read markdown {}: {error}", path.to_string_lossy()))
    })?;
    if !text.starts_with("---") {
        return Ok((BTreeMap::new(), text.trim().to_string()));
    }
    let mut lines = text.lines();
    let _ = lines.next();
    let mut raw_frontmatter = Vec::new();
    for line in &mut lines {
        if line.trim() == "---" {
            let frontmatter = parse_frontmatter_entries(&raw_frontmatter, path)?;
            return Ok((
                frontmatter,
                lines.collect::<Vec<_>>().join("\n").trim().to_string(),
            ));
        }
        raw_frontmatter.push(line.to_string());
    }
    Err(AgentOsError::Validation(format!(
        "{} has unterminated frontmatter",
        path.to_string_lossy()
    )))
}

fn parse_frontmatter_entries(
    lines: &[String],
    path: &Path,
) -> AgentOsResult<BTreeMap<String, String>> {
    let mut frontmatter = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        let line = &lines[index];
        if line.trim().is_empty() || line.starts_with(' ') || line.starts_with('\t') {
            index += 1;
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(AgentOsError::Validation(format!(
                "{} frontmatter only supports key: value entries",
                path.to_string_lossy()
            )));
        };
        let key = key.trim().to_string();
        let value = value.trim();
        if value.starts_with('|') || value.starts_with('>') {
            let mut block = Vec::new();
            index += 1;
            while index < lines.len()
                && (lines[index].starts_with(' ')
                    || lines[index].starts_with('\t')
                    || lines[index].trim().is_empty())
            {
                block.push(lines[index].trim().to_string());
                index += 1;
            }
            frontmatter.insert(key, block.join("\n").trim().to_string());
            continue;
        }
        frontmatter.insert(key, trim_quotes(value));
        index += 1;
    }
    Ok(frontmatter)
}

fn markdown_files(root: &Path) -> AgentOsResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_markdown_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn markdown_files_named(root: &Path, name: &str) -> AgentOsResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_named(root, name, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files(root: &Path, files: &mut Vec<PathBuf>) -> AgentOsResult<()> {
    for entry in fs::read_dir(root)
        .map_err(|error| AgentOsError::Validation(format!("read markdown dir: {error}")))?
    {
        let path = entry
            .map_err(|error| AgentOsError::Validation(format!("read markdown entry: {error}")))?
            .path();
        if path.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(())
}

fn collect_files_named(root: &Path, name: &str, files: &mut Vec<PathBuf>) -> AgentOsResult<()> {
    for entry in fs::read_dir(root)
        .map_err(|error| AgentOsError::Validation(format!("read ecosystem dir: {error}")))?
    {
        let path = entry
            .map_err(|error| AgentOsError::Validation(format!("read ecosystem entry: {error}")))?
            .path();
        if path.is_dir() {
            collect_files_named(&path, name, files)?;
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            files.push(path);
        }
    }
    Ok(())
}

fn relative_without_extension(root: &Path, path: &Path) -> AgentOsResult<String> {
    let rel = path.strip_prefix(root).map_err(|error| {
        AgentOsError::Validation(format!("compute relative markdown path: {error}"))
    })?;
    let mut text = rel.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_suffix(".md") {
        text = stripped.to_string();
    }
    Ok(text)
}

fn optional_frontmatter(frontmatter: &BTreeMap<String, String>, key: &str) -> Option<String> {
    frontmatter
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn markdown_description(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" {
            return None;
        }
        Some(trimmed.trim_start_matches('#').trim().to_string()).filter(|value| !value.is_empty())
    })
}

fn command_hints(template: &str) -> Vec<String> {
    let mut hints = Vec::new();
    if template.contains("$ARGUMENTS") {
        hints.push("$ARGUMENTS".to_string());
    }
    for index in 1..=9 {
        let token = format!("${index}");
        if template.contains(&token) {
            hints.push(token);
        }
    }
    hints
}

fn string_array_field(value: &Value, field: &str) -> AgentOsResult<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| AgentOsError::Validation(format!("{field} entries must be strings")))
        })
        .collect()
}

fn string_map_field(value: &Value, field: &str) -> AgentOsResult<BTreeMap<String, String>> {
    let Some(map) = value.get(field) else {
        return Ok(BTreeMap::new());
    };
    map.as_object()
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be an object")))?
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_string()))
                .ok_or_else(|| AgentOsError::Validation(format!("{field} values must be strings")))
        })
        .collect()
}

fn trim_quotes(value: &str) -> String {
    value.trim_matches('"').trim_matches('\'').to_string()
}

fn hash_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stable_id(prefix: &str, path: &Path, salt: &str) -> String {
    let hash = hash_text(&format!("{}\n{salt}", path.to_string_lossy()));
    format!("{prefix}_{}", &hash[..16])
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ecosystem_uses_project_agent_os_over_agents_and_claude_for_same_skill() {
        let root = temp_dir("agent-os-ecosystem-precedence");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join("config")).unwrap();
        fs::create_dir_all(workspace.join(".claude/skills/dupe")).unwrap();
        fs::create_dir_all(workspace.join(".agents/skills/dupe")).unwrap();
        fs::create_dir_all(workspace.join(".agent-os/skills/dupe")).unwrap();
        write_skill(
            &workspace.join(".claude/skills/dupe/SKILL.md"),
            "dupe",
            "claude",
        );
        write_skill(
            &workspace.join(".agents/skills/dupe/SKILL.md"),
            "dupe",
            "agents",
        );
        write_skill(
            &workspace.join(".agent-os/skills/dupe/SKILL.md"),
            "dupe",
            "agent-os",
        );

        let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
            workspace_root: workspace.clone(),
            paths: test_paths(&home),
        })
        .unwrap();

        let dupe = catalog
            .skill_definitions
            .iter()
            .find(|skill| skill.name == "dupe")
            .unwrap();
        assert_eq!(dupe.description, "agent-os skill.");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ecosystem_collects_global_and_project_instructions() {
        let root = temp_dir("agent-os-ecosystem-instructions");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join("config")).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        fs::write(home.join("config/AGENTS.md"), "global instruction\n").unwrap();
        fs::write(workspace.join("CLAUDE.md"), "project claude\n").unwrap();
        fs::write(workspace.join("AGENTS.md"), "project agents\n").unwrap();

        let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
            workspace_root: workspace.clone(),
            paths: test_paths(&home),
        })
        .unwrap();
        let body = catalog
            .instruction_documents
            .iter()
            .map(|document| document.content.as_str())
            .collect::<Vec<_>>()
            .join("");

        assert!(body.contains("global instruction"));
        assert!(body.contains("project claude"));
        assert!(body.contains("project agents"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_report_records_agents_and_claude_migration_sources() {
        let root = temp_dir("agent-os-ecosystem-source-report");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join("config")).unwrap();
        fs::create_dir_all(workspace.join(".claude/skills/claude-only")).unwrap();
        fs::create_dir_all(workspace.join(".agents/skills/agents-only")).unwrap();
        fs::write(workspace.join("CLAUDE.md"), "claude migration rule\n").unwrap();
        fs::write(workspace.join("AGENTS.md"), "agents project rule\n").unwrap();
        write_skill(
            &workspace.join(".claude/skills/claude-only/SKILL.md"),
            "claude-only",
            "claude migration",
        );
        write_skill(
            &workspace.join(".agents/skills/agents-only/SKILL.md"),
            "agents-only",
            "agents migration",
        );

        let catalog = discover_ecosystem(&EcosystemDiscoverOptions {
            workspace_root: workspace.clone(),
            paths: test_paths(&home),
        })
        .unwrap();
        let report = catalog.import_report();

        assert!(catalog
            .skill_definitions
            .iter()
            .any(|skill| skill.name == "claude-only"));
        assert!(catalog
            .skill_definitions
            .iter()
            .any(|skill| skill.name == "agents-only"));
        assert!(report.sources.iter().any(|source| {
            source.source_kind == EcosystemSourceKind::Claude
                && source.source_scope == EcosystemSourceScope::Project
                && source.source_path.ends_with("CLAUDE.md")
                && source.instructions == 1
        }));
        assert!(report.sources.iter().any(|source| {
            source.source_kind == EcosystemSourceKind::Agents
                && source.source_scope == EcosystemSourceScope::Project
                && source.source_path.ends_with("AGENTS.md")
                && source.instructions == 1
        }));
        assert!(report.sources.iter().any(|source| {
            source.source_kind == EcosystemSourceKind::Claude
                && source.source_scope == EcosystemSourceScope::Project
                && source.source_path.ends_with("SKILL.md")
                && source.skills == 1
        }));
        assert!(report.sources.iter().any(|source| {
            source.source_kind == EcosystemSourceKind::Agents
                && source.source_scope == EcosystemSourceScope::Project
                && source.source_path.ends_with("SKILL.md")
                && source.skills == 1
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn command_templates_expand_arguments() {
        assert_eq!(
            expand_command_template(
                "Review $1 with $ARGUMENTS and $2.",
                &["src/lib.rs".to_string(), "tests".to_string()],
                "src/lib.rs tests"
            ),
            "Review src/lib.rs with src/lib.rs tests and tests."
        );
    }

    fn write_skill(path: &Path, name: &str, marker: &str) {
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: {marker} skill.\n---\n{marker} body\n"),
        )
        .unwrap();
    }

    fn test_paths(home: &Path) -> AgentOsPaths {
        AgentOsPaths {
            home: home.to_path_buf(),
            config_dir: home.join("config"),
            data_dir: home.join("data"),
            state_dir: home.join("state"),
            cache_dir: home.join("cache"),
            log_dir: home.join("log"),
            bin_dir: home.join("cache/bin"),
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
