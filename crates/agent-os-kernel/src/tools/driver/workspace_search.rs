use crate::util::required_string;
use agent_os_sys::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs::{self, Metadata};
use std::path::Path;

#[derive(Debug)]
pub(super) struct SearchRequest {
    pub query: String,
    pub path: Option<String>,
    mode: SearchMode,
    case_sensitive: bool,
    offset: usize,
    limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Path,
    Content,
    Both,
}

impl SearchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Content => "content",
            Self::Both => "both",
        }
    }

    fn searches_path(self) -> bool {
        matches!(self, Self::Path | Self::Both)
    }

    fn searches_content(self) -> bool {
        matches!(self, Self::Content | Self::Both)
    }
}

struct SearchNeedle {
    raw: String,
    normalized: String,
    case_sensitive: bool,
}

impl SearchNeedle {
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

#[derive(Default)]
struct WorkspaceSearchResult {
    matches: Vec<SearchMatch>,
    files_searched: usize,
    files_skipped: usize,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SearchMatch {
    path: String,
    match_kind: &'static str,
    line_number: Option<usize>,
    line: Option<String>,
}

pub(super) fn parse_search_request(input: &Value) -> AgentOsResult<SearchRequest> {
    let query = required_string(input, "query")?;
    if query.trim().is_empty() {
        return Err(AgentOsError::Validation(
            "search_files query must not be empty".to_string(),
        ));
    }
    let path = optional_string(input, "path")?;
    if path.as_deref().is_some_and(str::is_empty) {
        return Err(AgentOsError::Validation(
            "search_files path must not be empty".to_string(),
        ));
    }
    let mode = match optional_string(input, "mode")?
        .unwrap_or_else(|| "both".to_string())
        .as_str()
    {
        "path" => SearchMode::Path,
        "content" => SearchMode::Content,
        "both" => SearchMode::Both,
        _ => {
            return Err(AgentOsError::Validation(
                "search_files mode must be path, content, or both".to_string(),
            ))
        }
    };
    let case_sensitive = optional_bool(input, "case_sensitive")?.unwrap_or(false);
    let offset = optional_usize(input, "offset")?.unwrap_or(0);
    let limit = optional_usize(input, "limit")?
        .unwrap_or(super::super::builtin::search_files::DEFAULT_LIMIT);
    if limit == 0 || limit > super::super::builtin::search_files::MAX_LIMIT {
        return Err(AgentOsError::Validation(format!(
            "search_files limit must be between 1 and {}",
            super::super::builtin::search_files::MAX_LIMIT
        )));
    }
    Ok(SearchRequest {
        query,
        path,
        mode,
        case_sensitive,
        offset,
        limit,
    })
}

pub(super) fn run_search(
    descriptor: &ToolDescriptor,
    input: &Value,
    root: &Path,
    scope: &Path,
    metadata: Metadata,
    request: SearchRequest,
) -> AgentOsResult<Value> {
    let mut result = WorkspaceSearchResult::default();
    let query = SearchNeedle::new(&request.query, request.case_sensitive);

    if metadata.is_file() {
        search_file(root, scope, &request, &query, &mut result)?;
    } else if metadata.is_dir() {
        search_directory(root, scope, &request, &query, &mut result)?;
    } else {
        return Err(AgentOsError::Validation(
            "search_files path must point to a file or directory".to_string(),
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
        "query": request.query,
        "mode": request.mode.as_str(),
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

fn search_directory(
    root: &Path,
    scope: &Path,
    request: &SearchRequest,
    query: &SearchNeedle,
    result: &mut WorkspaceSearchResult,
) -> AgentOsResult<()> {
    let mut directories = vec![scope.to_path_buf()];
    while let Some(directory) = directories.pop() {
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
            if file_type.is_dir() {
                if is_ignored_search_dir(&path) {
                    result.files_skipped += 1;
                } else {
                    directories.push(path);
                    directories.sort_by(|left, right| right.cmp(left));
                }
                continue;
            }
            if file_type.is_file() {
                search_file(root, &path, request, query, result)?;
                if result.truncated {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn search_file(
    root: &Path,
    path: &Path,
    request: &SearchRequest,
    query: &SearchNeedle,
    result: &mut WorkspaceSearchResult,
) -> AgentOsResult<()> {
    if result.files_searched >= super::super::builtin::search_files::MAX_VISITED_FILES {
        result.truncated = true;
        return Ok(());
    }
    result.files_searched += 1;
    let relative_path = workspace_relative_path(root, path);
    if request.mode.searches_path() && query.matches(&relative_path) {
        push_search_match(
            result,
            SearchMatch {
                path: relative_path.clone(),
                match_kind: "path",
                line_number: None,
                line: None,
            },
        );
    }
    if !request.mode.searches_content() || result.truncated {
        return Ok(());
    }
    let metadata = fs::metadata(path)
        .map_err(|error| AgentOsError::Validation(format!("stat workspace file: {error}")))?;
    if metadata.len() > super::super::builtin::search_files::MAX_FILE_BYTES {
        result.files_skipped += 1;
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|error| {
        AgentOsError::Validation(format!("read workspace search file: {error}"))
    })?;
    let Ok(content) = String::from_utf8(bytes) else {
        result.files_skipped += 1;
        return Ok(());
    };
    for (index, line) in content.lines().enumerate() {
        if query.matches(line) {
            push_search_match(
                result,
                SearchMatch {
                    path: relative_path.clone(),
                    match_kind: "content",
                    line_number: Some(index + 1),
                    line: Some(preview_search_line(line)),
                },
            );
            if result.truncated {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn push_search_match(result: &mut WorkspaceSearchResult, item: SearchMatch) {
    if result.matches.len() >= super::super::builtin::search_files::MAX_RESULTS {
        result.truncated = true;
        return;
    }
    result.matches.push(item);
    if result.matches.len() >= super::super::builtin::search_files::MAX_RESULTS {
        result.truncated = true;
    }
}

fn is_ignored_search_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules"))
}

fn workspace_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn preview_search_line(line: &str) -> String {
    line.chars().take(500).collect()
}

fn normalize_search_text(text: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    }
}

fn optional_usize(input: &Value, field: &str) -> AgentOsResult<Option<usize>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| {
            AgentOsError::Validation(format!(
                "search_files {field} must be a non-negative integer"
            ))
        })
}

fn optional_bool(input: &Value, field: &str) -> AgentOsResult<Option<bool>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| AgentOsError::Validation(format!("search_files {field} must be a boolean")))
}

fn optional_string(input: &Value, field: &str) -> AgentOsResult<Option<String>> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| AgentOsError::Validation(format!("search_files {field} must be a string")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_search_finds_content_and_keeps_agent_dirs_searchable() {
        let root = std::env::temp_dir().join(format!(
            "agent-os-search-unit-{}-{}",
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

        let request = parse_search_request(&json!({
            "query": "searchneedle",
            "mode": "both",
            "limit": 20
        }))
        .unwrap();
        let query = SearchNeedle::new(&request.query, request.case_sensitive);
        let mut result = WorkspaceSearchResult::default();
        search_directory(&root, &root, &request, &query, &mut result).unwrap();

        let matches = result
            .matches
            .iter()
            .map(|item| (item.path.as_str(), item.match_kind))
            .collect::<Vec<_>>();
        assert!(matches.contains(&("src/lib.rs", "content")));
        assert!(matches.contains(&(".agents/skill.md", "content")));
        assert!(!matches.iter().any(|(path, _)| path.starts_with(".git/")));
        assert!(result.files_skipped >= 1);

        let _ = fs::remove_dir_all(root);
    }
}
