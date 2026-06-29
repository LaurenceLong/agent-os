use super::{hash_text, home_dir, stable_id};
use agent_os_sys::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn read_agent_os_config(path: &Path) -> AgentOsResult<Value> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    serde_json::from_str(
        &fs::read_to_string(path)
            .map_err(|error| AgentOsError::Validation(format!("read agent-os.json: {error}")))?,
    )
    .map_err(AgentOsError::from)
}

pub(super) fn discover_instruction_documents(
    workspace_root: &Path,
    config: &Value,
) -> AgentOsResult<Vec<InstructionDocument>> {
    let mut documents = Vec::new();
    if let Some(path) = nearest_instruction_file(workspace_root) {
        documents.push(instruction_document(
            &path,
            source_for_instruction(&path, EcosystemSourceScope::Project),
            0,
        )?);
    }
    if let Some(home) = home_dir() {
        for (rank, (path, kind)) in [
            (
                home.join(".config/agent-os/AGENTS.md"),
                EcosystemSourceKind::AgentOs,
            ),
            (home.join(".claude/CLAUDE.md"), EcosystemSourceKind::Claude),
        ]
        .into_iter()
        .enumerate()
        {
            if path.is_file() {
                documents.push(instruction_document(
                    &path,
                    EcosystemSource {
                        source_kind: kind,
                        source_scope: EcosystemSourceScope::Global,
                        source_path: path.to_string_lossy().to_string(),
                    },
                    100 + rank as u32,
                )?);
            }
        }
    }
    if let Some(items) = config.get("instructions").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            let raw = item.as_str().ok_or_else(|| {
                AgentOsError::Validation(
                    "agent-os.json instructions entries must be strings".to_string(),
                )
            })?;
            if raw.starts_with("http://") || raw.starts_with("https://") || raw.contains('*') {
                return Err(AgentOsError::Validation(
                    "agent-os.json instructions support exact local paths only".to_string(),
                ));
            }
            let path = if Path::new(raw).is_absolute() {
                PathBuf::from(raw)
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
                    200 + index as u32,
                )?);
            }
        }
    }
    Ok(documents)
}

pub(super) fn discover_skills(workspace_root: &Path) -> AgentOsResult<Vec<SkillDefinition>> {
    let mut skills = Vec::new();
    for root in ecosystem_roots(workspace_root) {
        let skills_root = root.path.join("skills");
        if !skills_root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&skills_root)
            .map_err(|error| AgentOsError::Validation(format!("read skills dir: {error}")))?
        {
            let skill_root = entry
                .map_err(|error| AgentOsError::Validation(format!("read skill entry: {error}")))?
                .path();
            let skill_file = skill_root.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
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

pub(super) fn discover_commands(workspace_root: &Path) -> AgentOsResult<Vec<CommandDefinition>> {
    let mut commands = Vec::new();
    for root in ecosystem_roots(workspace_root).into_iter().filter(|root| {
        matches!(
            root.kind,
            EcosystemSourceKind::AgentOs | EcosystemSourceKind::OpenCode
        )
    }) {
        let command_root = root.path.join("commands");
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

pub(super) fn discover_agent_profiles(
    workspace_root: &Path,
) -> AgentOsResult<Vec<ImportedAgentProfile>> {
    let mut profiles = Vec::new();
    for root in ecosystem_roots(workspace_root).into_iter().filter(|root| {
        matches!(
            root.kind,
            EcosystemSourceKind::AgentOs | EcosystemSourceKind::OpenCode
        )
    }) {
        let agent_root = root.path.join("agents");
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
                    )));
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

pub(super) struct EcosystemRoot {
    path: PathBuf,
    kind: EcosystemSourceKind,
    scope: EcosystemSourceScope,
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

pub(super) fn ecosystem_roots(workspace_root: &Path) -> Vec<EcosystemRoot> {
    let mut roots = vec![
        root(
            workspace_root.join(".agent-os"),
            EcosystemSourceKind::AgentOs,
            EcosystemSourceScope::Project,
        ),
        root(
            workspace_root.join(".opencode"),
            EcosystemSourceKind::OpenCode,
            EcosystemSourceScope::Project,
        ),
        root(
            workspace_root.join(".agents"),
            EcosystemSourceKind::Agents,
            EcosystemSourceScope::Project,
        ),
        root(
            workspace_root.join(".claude"),
            EcosystemSourceKind::Claude,
            EcosystemSourceScope::Project,
        ),
    ];
    if let Some(home) = home_dir() {
        roots.extend([
            root(
                home.join(".config/agent-os"),
                EcosystemSourceKind::AgentOs,
                EcosystemSourceScope::Global,
            ),
            root(
                home.join(".config/opencode"),
                EcosystemSourceKind::OpenCode,
                EcosystemSourceScope::Global,
            ),
            root(
                home.join(".agents"),
                EcosystemSourceKind::Agents,
                EcosystemSourceScope::Global,
            ),
            root(
                home.join(".claude"),
                EcosystemSourceKind::Claude,
                EcosystemSourceScope::Global,
            ),
        ]);
    }
    roots
}

fn root(path: PathBuf, kind: EcosystemSourceKind, scope: EcosystemSourceScope) -> EcosystemRoot {
    EcosystemRoot { path, kind, scope }
}

fn nearest_instruction_file(start: &Path) -> Option<PathBuf> {
    for current in start.ancestors() {
        let agents = current.join("AGENTS.md");
        if agents.is_file() {
            return Some(agents);
        }
        let claude = current.join("CLAUDE.md");
        if claude.is_file() {
            return Some(claude);
        }
    }
    None
}

fn source_for_instruction(path: &Path, source_scope: EcosystemSourceScope) -> EcosystemSource {
    let source_kind = match path.file_name().and_then(|name| name.to_str()) {
        Some(file) if file.eq_ignore_ascii_case("CLAUDE.md") => EcosystemSourceKind::Claude,
        _ => EcosystemSourceKind::AgentOs,
    };
    EcosystemSource {
        source_kind,
        source_scope,
        source_path: path.to_string_lossy().to_string(),
    }
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

fn trim_quotes(value: &str) -> String {
    value.trim_matches('"').trim_matches('\'').to_string()
}
