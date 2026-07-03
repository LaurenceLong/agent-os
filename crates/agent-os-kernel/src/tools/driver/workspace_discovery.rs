use crate::util::required_string;
use agent_os_sys::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, Metadata};
use std::path::{Component, Path};

#[derive(Debug)]
pub(super) struct GlobRequest {
    pub pattern: String,
    pub path: Option<String>,
    offset: usize,
    limit: usize,
}

#[derive(Debug)]
pub(super) struct GrepRequest {
    pub pattern: String,
    pub path: Option<String>,
    include: Option<String>,
    case_sensitive: bool,
    offset: usize,
    limit: usize,
}

struct DiscoveryResult<T> {
    matches: Vec<T>,
    files_searched: usize,
    files_skipped: usize,
    truncated: bool,
}

impl<T> Default for DiscoveryResult<T> {
    fn default() -> Self {
        Self {
            matches: Vec::new(),
            files_searched: 0,
            files_skipped: 0,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GlobMatch {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct GrepMatch {
    path: String,
    line_number: usize,
    line: String,
}

struct LiteralNeedle {
    raw: String,
    normalized: String,
    case_sensitive: bool,
}

impl LiteralNeedle {
    fn new(raw: &str, case_sensitive: bool) -> Self {
        Self {
            raw: raw.to_string(),
            normalized: normalize_search_text(raw, case_sensitive),
            case_sensitive,
        }
    }

    fn matches(&self, text: &str) -> bool {
        if self.case_sensitive {
            text.contains(&self.raw)
        } else {
            normalize_search_text(text, false).contains(&self.normalized)
        }
    }
}

pub(super) fn parse_glob_request(input: &Value) -> AgentOsResult<GlobRequest> {
    let pattern = required_string(input, "pattern")?;
    validate_pattern("glob_files pattern", &pattern)?;
    let path = optional_string(input, "path")?;
    validate_optional_path("glob_files path", path.as_deref())?;
    let offset = optional_usize(input, "offset", "glob_files")?.unwrap_or(0);
    let limit = optional_usize(input, "limit", "glob_files")?
        .unwrap_or(super::super::builtin::glob_files::DEFAULT_LIMIT);
    if limit == 0 || limit > super::super::builtin::glob_files::MAX_LIMIT {
        return Err(AgentOsError::Validation(format!(
            "glob_files limit must be between 1 and {}",
            super::super::builtin::glob_files::MAX_LIMIT
        )));
    }
    Ok(GlobRequest {
        pattern: normalize_pattern(&pattern),
        path,
        offset,
        limit,
    })
}

pub(super) fn parse_grep_request(input: &Value) -> AgentOsResult<GrepRequest> {
    let pattern = required_string(input, "pattern")?;
    validate_pattern("grep_files pattern", &pattern)?;
    let path = optional_string(input, "path")?;
    validate_optional_path("grep_files path", path.as_deref())?;
    let include = optional_string(input, "include")?.map(|pattern| normalize_pattern(&pattern));
    if let Some(include) = include.as_deref() {
        validate_pattern("grep_files include", include)?;
    }
    let case_sensitive = optional_bool(input, "case_sensitive", "grep_files")?.unwrap_or(false);
    let offset = optional_usize(input, "offset", "grep_files")?.unwrap_or(0);
    let limit = optional_usize(input, "limit", "grep_files")?
        .unwrap_or(super::super::builtin::grep_files::DEFAULT_LIMIT);
    if limit == 0 || limit > super::super::builtin::grep_files::MAX_LIMIT {
        return Err(AgentOsError::Validation(format!(
            "grep_files limit must be between 1 and {}",
            super::super::builtin::grep_files::MAX_LIMIT
        )));
    }
    Ok(GrepRequest {
        pattern: pattern.to_string(),
        path,
        include,
        case_sensitive,
        offset,
        limit,
    })
}

pub(super) fn run_glob(
    descriptor: &ToolDescriptor,
    input: &Value,
    root: &Path,
    scope: &Path,
    metadata: Metadata,
    request: GlobRequest,
) -> AgentOsResult<Value> {
    if !metadata.is_dir() {
        return Err(AgentOsError::Validation(
            "glob_files path must point to a directory".to_string(),
        ));
    }
    let mut result = DiscoveryResult::<GlobMatch>::default();
    walk_files(
        root,
        scope,
        scope,
        &mut result,
        |root, scope, file, result| {
            if result.files_searched >= super::super::builtin::glob_files::MAX_VISITED_FILES {
                result.truncated = true;
                return Ok(());
            }
            result.files_searched += 1;
            let scoped_path = scoped_relative_path(scope, file);
            if glob_matches(&request.pattern, &scoped_path) {
                push_match(
                    result,
                    GlobMatch {
                        path: workspace_relative_path(root, file),
                    },
                    super::super::builtin::glob_files::MAX_RESULTS,
                );
            }
            Ok(())
        },
    )?;

    let total_matches = result.matches.len();
    let start = request.offset.min(total_matches);
    let end = start.saturating_add(request.limit).min(total_matches);
    let returned = result.matches[start..end].to_vec();
    let next_offset = (end < total_matches).then_some(end);
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "pattern": request.pattern,
        "path": request.path.unwrap_or_else(|| ".".to_string()),
        "offset": request.offset,
        "limit": request.limit,
        "total_matches": total_matches,
        "returned_matches": returned.len(),
        "next_offset": next_offset,
        "matches": returned,
        "truncated": result.truncated,
        "files_searched": result.files_searched,
        "files_skipped": result.files_skipped,
    }))
}

pub(super) fn run_grep(
    descriptor: &ToolDescriptor,
    input: &Value,
    root: &Path,
    scope: &Path,
    metadata: Metadata,
    request: GrepRequest,
) -> AgentOsResult<Value> {
    let mut result = DiscoveryResult::<GrepMatch>::default();
    let needle = LiteralNeedle::new(&request.pattern, request.case_sensitive);

    if metadata.is_file() {
        grep_file(root, scope, scope, &request, &needle, &mut result)?;
    } else if metadata.is_dir() {
        walk_files(
            root,
            scope,
            scope,
            &mut result,
            |root, scope, file, result| grep_file(root, scope, file, &request, &needle, result),
        )?;
    } else {
        return Err(AgentOsError::Validation(
            "grep_files path must point to a file or directory".to_string(),
        ));
    }

    let total_matches = result.matches.len();
    let start = request.offset.min(total_matches);
    let end = start.saturating_add(request.limit).min(total_matches);
    let returned = result.matches[start..end].to_vec();
    let next_offset = (end < total_matches).then_some(end);
    Ok(json!({
        "tool": descriptor.name.clone(),
        "status": "ok",
        "input": input.clone(),
        "driver_class": descriptor.driver_class,
        "pattern": request.pattern,
        "path": request.path.unwrap_or_else(|| ".".to_string()),
        "include": request.include,
        "case_sensitive": request.case_sensitive,
        "offset": request.offset,
        "limit": request.limit,
        "total_matches": total_matches,
        "returned_matches": returned.len(),
        "next_offset": next_offset,
        "matches": returned,
        "truncated": result.truncated,
        "files_searched": result.files_searched,
        "files_skipped": result.files_skipped,
    }))
}

fn walk_files<T>(
    root: &Path,
    scope: &Path,
    start: &Path,
    result: &mut DiscoveryResult<T>,
    mut on_file: impl FnMut(&Path, &Path, &Path, &mut DiscoveryResult<T>) -> AgentOsResult<()>,
) -> AgentOsResult<()> {
    let start_rules = load_gitignore_rules_to_scope(root, start)?;
    let mut directories = vec![(start.to_path_buf(), start_rules)];
    while let Some((directory, rules)) = directories.pop() {
        let mut children = fs::read_dir(&directory)
            .map_err(|error| {
                AgentOsError::Validation(format!("read workspace directory: {error}"))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AgentOsError::Validation(format!("read workspace directory entry: {error}"))
            })?;
        children.sort_by_key(|entry| entry.path());
        for entry in children {
            let file_type = entry.file_type().map_err(|error| {
                AgentOsError::Validation(format!("stat workspace directory entry: {error}"))
            })?;
            if file_type.is_symlink() {
                result.files_skipped += 1;
                continue;
            }
            let path = entry.path();
            let ignored = is_ignored_discovery_path(root, &path, file_type.is_dir(), &rules);
            if file_type.is_dir() {
                if ignored {
                    result.files_skipped += 1;
                } else {
                    let child_rules = load_gitignore_rules(root, &path, &rules)?;
                    directories.push((path, child_rules));
                    directories.sort_by(|left, right| right.0.cmp(&left.0));
                }
                continue;
            }
            if file_type.is_file() {
                if ignored {
                    result.files_skipped += 1;
                    continue;
                }
                on_file(root, scope, &path, result)?;
                if result.truncated {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn grep_file(
    root: &Path,
    scope: &Path,
    path: &Path,
    request: &GrepRequest,
    needle: &LiteralNeedle,
    result: &mut DiscoveryResult<GrepMatch>,
) -> AgentOsResult<()> {
    if result.files_searched >= super::super::builtin::grep_files::MAX_VISITED_FILES {
        result.truncated = true;
        return Ok(());
    }
    let scoped_path = scoped_relative_path(scope, path);
    if request
        .include
        .as_deref()
        .is_some_and(|include| !glob_matches(include, &scoped_path))
    {
        return Ok(());
    }
    result.files_searched += 1;
    let metadata = fs::metadata(path)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace file: {error}")))?;
    if metadata.len() > super::super::builtin::grep_files::MAX_FILE_BYTES {
        result.files_skipped += 1;
        return Ok(());
    }
    let bytes = fs::read(path)
        .map_err(|error| AgentOsError::Validation(format!("read workspace grep file: {error}")))?;
    let Ok(content) = String::from_utf8(bytes) else {
        result.files_skipped += 1;
        return Ok(());
    };
    let relative_path = workspace_relative_path(root, path);
    for (index, line) in content.lines().enumerate() {
        if needle.matches(line) {
            push_match(
                result,
                GrepMatch {
                    path: relative_path.clone(),
                    line_number: index + 1,
                    line: preview_line(line),
                },
                super::super::builtin::grep_files::MAX_RESULTS,
            );
            if result.truncated {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn push_match<T>(result: &mut DiscoveryResult<T>, item: T, max_results: usize) {
    if result.matches.len() >= max_results {
        result.truncated = true;
        return;
    }
    result.matches.push(item);
    if result.matches.len() >= max_results {
        result.truncated = true;
    }
}

#[derive(Debug, Clone)]
struct GitignoreRule {
    base: String,
    pattern: String,
    negated: bool,
    directory_only: bool,
    anchored: bool,
}

fn load_gitignore_rules_to_scope(root: &Path, scope: &Path) -> AgentOsResult<Vec<GitignoreRule>> {
    let mut rules = load_gitignore_rules(root, root, &[])?;
    let relative = scope.strip_prefix(root).unwrap_or(scope);
    let mut directory = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        directory.push(name);
        if directory == root {
            continue;
        }
        rules = load_gitignore_rules(root, &directory, &rules)?;
    }
    Ok(rules)
}

fn load_gitignore_rules(
    root: &Path,
    directory: &Path,
    inherited: &[GitignoreRule],
) -> AgentOsResult<Vec<GitignoreRule>> {
    let mut rules = inherited.to_vec();
    let gitignore = directory.join(".gitignore");
    if !gitignore.exists() {
        return Ok(rules);
    }
    let content = fs::read_to_string(&gitignore)
        .map_err(|error| AgentOsError::Validation(format!("read .gitignore: {error}")))?;
    let base = workspace_relative_path(root, directory);
    for line in content.lines() {
        let Some(rule) = parse_gitignore_rule(&base, line) else {
            continue;
        };
        rules.push(rule);
    }
    Ok(rules)
}

fn parse_gitignore_rule(base: &str, line: &str) -> Option<GitignoreRule> {
    let mut pattern = line.trim_end_matches('\r').trim_end();
    if pattern.is_empty() || pattern.starts_with('#') {
        return None;
    }
    let negated = pattern.starts_with('!');
    if negated {
        pattern = pattern[1..].trim_start();
    }
    let anchored = pattern.starts_with('/');
    if anchored {
        pattern = &pattern[1..];
    }
    let directory_only = pattern.ends_with('/');
    if directory_only {
        pattern = pattern.trim_end_matches('/');
    }
    if pattern.is_empty() {
        return None;
    }
    Some(GitignoreRule {
        base: base.to_string(),
        pattern: normalize_pattern(pattern),
        negated,
        directory_only,
        anchored,
    })
}

fn is_ignored_discovery_path(
    root: &Path,
    path: &Path,
    is_dir: bool,
    rules: &[GitignoreRule],
) -> bool {
    if is_dir && is_builtin_ignored_dir(path) {
        return true;
    }
    let relative = workspace_relative_path(root, path);
    let mut ignored = false;
    for rule in rules {
        if gitignore_rule_matches(rule, &relative, is_dir) {
            ignored = !rule.negated;
        }
    }
    ignored
}

fn is_builtin_ignored_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules"))
}

fn gitignore_rule_matches(rule: &GitignoreRule, relative: &str, is_dir: bool) -> bool {
    if rule.directory_only && !is_dir {
        return false;
    }
    let Some(local) = path_under_rule_base(&rule.base, relative) else {
        return false;
    };
    if rule.anchored || rule.pattern.contains('/') {
        return glob_matches(&rule.pattern, local);
    }
    local
        .split('/')
        .any(|component| glob_matches(&rule.pattern, component))
}

fn path_under_rule_base<'a>(base: &str, relative: &'a str) -> Option<&'a str> {
    if base.is_empty() {
        return Some(relative);
    }
    if relative == base {
        return Some("");
    }
    relative
        .strip_prefix(base)
        .and_then(|suffix| suffix.strip_prefix('/'))
}

fn workspace_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn scoped_relative_path(scope: &Path, path: &Path) -> String {
    path.strip_prefix(scope)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches(['\\', '/'])
        .replace('\\', "/")
}

fn preview_line(line: &str) -> String {
    line.chars().take(500).collect()
}

fn normalize_search_text(text: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    }
}

fn normalize_pattern(pattern: &str) -> String {
    pattern.replace('\\', "/")
}

fn validate_pattern(field: &str, pattern: &str) -> AgentOsResult<()> {
    if pattern.trim().is_empty() {
        return Err(AgentOsError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    if pattern.starts_with('/') || pattern.contains("../") || pattern == ".." {
        return Err(AgentOsError::Validation(format!(
            "{field} must be workspace-relative"
        )));
    }
    Ok(())
}

fn validate_optional_path(field: &str, path: Option<&str>) -> AgentOsResult<()> {
    if path.is_some_and(str::is_empty) {
        return Err(AgentOsError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn optional_usize(input: &Value, field: &str, tool: &str) -> AgentOsResult<Option<usize>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| {
            AgentOsError::Validation(format!("{tool} {field} must be a non-negative integer"))
        })
}

fn optional_bool(input: &Value, field: &str, tool: &str) -> AgentOsResult<Option<bool>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| AgentOsError::Validation(format!("{tool} {field} must be a boolean")))
}

fn optional_string(input: &Value, field: &str) -> AgentOsResult<Option<String>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| AgentOsError::Validation(format!("{field} must be a string")))
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    brace_expand(pattern)
        .iter()
        .any(|expanded| glob_matches_one(expanded.as_bytes(), text.as_bytes()))
}

fn brace_expand(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let Some(close) = pattern[open + 1..].find('}').map(|index| open + 1 + index) else {
        return vec![pattern.to_string()];
    };
    let prefix = &pattern[..open];
    let suffix = &pattern[close + 1..];
    pattern[open + 1..close]
        .split(',')
        .take(32)
        .flat_map(|part| brace_expand(&format!("{prefix}{part}{suffix}")))
        .take(64)
        .collect()
}

fn glob_matches_one(pattern: &[u8], text: &[u8]) -> bool {
    fn inner(
        pattern: &[u8],
        text: &[u8],
        pi: usize,
        ti: usize,
        memo: &mut HashMap<(usize, usize), bool>,
    ) -> bool {
        if let Some(value) = memo.get(&(pi, ti)) {
            return *value;
        }
        let value = if pi == pattern.len() {
            ti == text.len()
        } else if pattern[pi] == b'*' {
            if pattern.get(pi + 1) == Some(&b'*') {
                let next = pi + 2;
                if pattern.get(next) == Some(&b'/') {
                    inner(pattern, text, next + 1, ti, memo)
                        || (ti < text.len() && inner(pattern, text, pi, ti + 1, memo))
                } else {
                    (ti..=text.len()).any(|next_ti| inner(pattern, text, next, next_ti, memo))
                }
            } else {
                let mut next_ti = ti;
                loop {
                    if inner(pattern, text, pi + 1, next_ti, memo) {
                        break true;
                    }
                    if next_ti >= text.len() || text[next_ti] == b'/' {
                        break false;
                    }
                    next_ti += 1;
                }
            }
        } else if pattern[pi] == b'?' {
            ti < text.len() && text[ti] != b'/' && inner(pattern, text, pi + 1, ti + 1, memo)
        } else {
            ti < text.len() && pattern[pi] == text[ti] && inner(pattern, text, pi + 1, ti + 1, memo)
        };
        memo.insert((pi, ti), value);
        value
    }

    inner(pattern, text, 0, 0, &mut HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_common_file_patterns() {
        assert!(glob_matches("**/*.rs", "src/lib.rs"));
        assert!(glob_matches("**/*.rs", "lib.rs"));
        assert!(glob_matches("crates/**/Cargo.toml", "crates/a/Cargo.toml"));
        assert!(glob_matches("*.{rs,toml}", "Cargo.toml"));
        assert!(!glob_matches("*.rs", "src/lib.rs"));
    }

    #[test]
    fn glob_and_grep_keep_discovery_deterministic_and_scoped() {
        let root = std::env::temp_dir().join(format!(
            "agent-os-discovery-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".agents")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub struct SearchNeedle;\n").unwrap();
        fs::write(root.join(".agents/skill.md"), "searchneedle lives here\n").unwrap();
        fs::write(
            root.join(".git/config"),
            "searchneedle should stay ignored\n",
        )
        .unwrap();

        let glob = parse_glob_request(&json!({
            "pattern": "**/*.rs",
            "limit": 20
        }))
        .unwrap();
        let glob_output = run_glob(
            &ToolDescriptor {
                tool_id: "test".to_string(),
                name: "glob_files".to_string(),
                ..ToolDescriptor::default()
            },
            &json!({}),
            &root,
            &root,
            fs::metadata(&root).unwrap(),
            glob,
        )
        .unwrap();
        assert_eq!(glob_output["matches"][0]["path"], "src/lib.rs");

        let grep = parse_grep_request(&json!({
            "pattern": "searchneedle",
            "include": "**/*.md",
            "limit": 20
        }))
        .unwrap();
        let grep_output = run_grep(
            &ToolDescriptor {
                tool_id: "test".to_string(),
                name: "grep_files".to_string(),
                ..ToolDescriptor::default()
            },
            &json!({}),
            &root,
            &root,
            fs::metadata(&root).unwrap(),
            grep,
        )
        .unwrap();
        assert_eq!(grep_output["matches"][0]["path"], ".agents/skill.md");
        assert_eq!(grep_output["files_skipped"], 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn glob_and_grep_apply_gitignore_rules_during_traversal() {
        let root = std::env::temp_dir().join(format!(
            "agent-os-discovery-gitignore-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::create_dir_all(root.join("build")).unwrap();
        fs::create_dir_all(root.join("nested/cache")).unwrap();
        fs::write(
            root.join(".gitignore"),
            "ignored.txt\nbuild/\ngenerated/*\n!generated/keep.txt\n",
        )
        .unwrap();
        fs::write(root.join("nested/.gitignore"), "cache/\n").unwrap();
        fs::write(root.join("visible.txt"), "needle visible\n").unwrap();
        fs::write(root.join("ignored.txt"), "needle ignored\n").unwrap();
        fs::write(root.join("build/hidden.txt"), "needle hidden\n").unwrap();
        fs::write(root.join("generated/drop.txt"), "needle drop\n").unwrap();
        fs::write(root.join("generated/keep.txt"), "needle keep\n").unwrap();
        fs::write(root.join("nested/cache/hidden.txt"), "needle cache\n").unwrap();

        let glob = parse_glob_request(&json!({
            "pattern": "**/*.txt",
            "limit": 20
        }))
        .unwrap();
        let glob_output = run_glob(
            &ToolDescriptor {
                tool_id: "test".to_string(),
                name: "glob_files".to_string(),
                ..ToolDescriptor::default()
            },
            &json!({}),
            &root,
            &root,
            fs::metadata(&root).unwrap(),
            glob,
        )
        .unwrap();
        let glob_paths = glob_output["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(glob_paths, vec!["visible.txt", "generated/keep.txt"]);
        assert_eq!(glob_output["files_skipped"], 4);

        let grep = parse_grep_request(&json!({
            "pattern": "needle",
            "include": "**/*.txt",
            "limit": 20
        }))
        .unwrap();
        let grep_output = run_grep(
            &ToolDescriptor {
                tool_id: "test".to_string(),
                name: "grep_files".to_string(),
                ..ToolDescriptor::default()
            },
            &json!({}),
            &root,
            &root,
            fs::metadata(&root).unwrap(),
            grep,
        )
        .unwrap();
        let grep_paths = grep_output["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(grep_paths, vec!["visible.txt", "generated/keep.txt"]);
        assert_eq!(grep_output["files_skipped"], 4);

        let _ = fs::remove_dir_all(root);
    }
}
