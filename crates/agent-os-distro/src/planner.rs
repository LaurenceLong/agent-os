use crate::types::{SoftwareCodeTask, SoftwareEditPlanSource};
use agent_os_sys::{AgentOsError, AgentOsResult};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) fn plan_from_task(
    workspace_root: impl Into<PathBuf>,
    task: impl Into<String>,
    scoped_file: Option<PathBuf>,
    test_program: impl Into<PathBuf>,
    test_args: Vec<String>,
) -> AgentOsResult<SoftwareCodeTask> {
    let workspace_root = workspace_root.into();
    let task = task.into();
    let (from_phrase, to_phrase) = parse_from_to(&task)?;
    let edit_pairs = value_candidate_pairs(&from_phrase, &to_phrase);
    let files = candidate_files(&workspace_root, scoped_file.as_deref())?;
    let mut matches = Vec::new();

    for file in files {
        let absolute = workspace_root.join(&file);
        let Ok(content) = fs::read_to_string(&absolute) else {
            continue;
        };
        for (old, new) in &edit_pairs {
            if old.is_empty() {
                continue;
            }
            let occurrences = content.matches(old).count();
            if occurrences != 1 {
                continue;
            }
            if old != new {
                matches.push(InferredEdit {
                    file: file.clone(),
                    old: old.clone(),
                    new: new.clone(),
                });
            }
        }
    }

    matches.sort_by(|left, right| {
        score_candidate(&task, left)
            .cmp(&score_candidate(&task, right))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.old.cmp(&right.old))
            .then_with(|| left.new.cmp(&right.new))
    });
    matches.dedup_by(|left, right| {
        left.file == right.file && left.old == right.old && left.new == right.new
    });
    let Some(best) = matches.first().cloned() else {
        return Err(AgentOsError::Validation(
            "could not infer a single safe edit from task; provide --file, --old, and --new"
                .to_string(),
        ));
    };
    let best_score = score_candidate(&task, &best);
    let equivalent_best = matches
        .iter()
        .filter(|candidate| score_candidate(&task, candidate) == best_score)
        .count();
    if equivalent_best != 1 {
        return Err(AgentOsError::Validation(
            "task inference found multiple equally safe edits; provide --file, --old, and --new"
                .to_string(),
        ));
    }

    let mut spec = SoftwareCodeTask::exact_edit(
        workspace_root,
        task,
        best.file,
        best.old,
        best.new,
        test_program,
        test_args,
    );
    spec.edit_plan_source = SoftwareEditPlanSource::Inferred;
    Ok(spec)
}

fn parse_from_to(task: &str) -> AgentOsResult<(String, String)> {
    let lower = task.to_ascii_lowercase();
    let from_index = lower.find(" from ").or_else(|| lower.find(" from\n"));
    let to_index = lower.find(" to ").or_else(|| lower.find(" to\n"));
    let (Some(from_index), Some(to_index)) = (from_index, to_index) else {
        return Err(AgentOsError::Validation(
            "task inference requires wording like `from X to Y`".to_string(),
        ));
    };
    if from_index >= to_index {
        return Err(AgentOsError::Validation(
            "task inference requires `from` before `to`".to_string(),
        ));
    }
    let from = task[from_index + 6..to_index].trim();
    let to_start = to_index + 4;
    let to = task[to_start..]
        .split(['.', ',', ';', '\n'])
        .next()
        .unwrap_or_default()
        .trim();
    if from.is_empty() || to.is_empty() {
        return Err(AgentOsError::Validation(
            "task inference requires non-empty `from` and `to` values".to_string(),
        ));
    }
    Ok((from.to_string(), to.to_string()))
}

fn value_candidate_pairs(from: &str, to: &str) -> Vec<(String, String)> {
    let from = from.trim().trim_matches('"').trim_matches('\'');
    let to = to.trim().trim_matches('"').trim_matches('\'');
    let mut pairs = vec![(from.to_string(), to.to_string())];
    if let (Some(from_number), Some(to_number)) = (word_number(from), word_number(to)) {
        pairs.push((from_number.to_string(), to_number.to_string()));
    }
    if let (Ok(from_number), Ok(to_number)) = (from.parse::<u8>(), to.parse::<u8>()) {
        if let (Some(from_word), Some(to_word)) = (number_word(from_number), number_word(to_number))
        {
            pairs.push((from_word.to_string(), to_word.to_string()));
        }
    }
    pairs.sort();
    pairs.dedup();
    pairs
}

fn candidate_files(
    workspace_root: &Path,
    scoped_file: Option<&Path>,
) -> AgentOsResult<Vec<PathBuf>> {
    if let Some(file) = scoped_file {
        ensure_safe_relative_path(file)?;
        return Ok(vec![file.to_path_buf()]);
    }
    let mut files = Vec::new();
    collect_candidate_files(workspace_root, workspace_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_candidate_files(
    workspace_root: &Path,
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> AgentOsResult<()> {
    for entry in fs::read_dir(dir)
        .map_err(|error| AgentOsError::Validation(format!("read workspace directory: {error}")))?
    {
        let entry = entry
            .map_err(|error| AgentOsError::Validation(format!("read workspace entry: {error}")))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name == ".git" || file_name == "target" || file_name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_candidate_files(workspace_root, &path, files)?;
            continue;
        }
        if !is_supported_source_file(&path) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| AgentOsError::Validation(format!("read metadata: {error}")))?;
        if metadata.len() > 1_000_000 {
            continue;
        }
        let relative = path.strip_prefix(workspace_root).map_err(|error| {
            AgentOsError::Validation(format!("derive relative workspace path: {error}"))
        })?;
        files.push(relative.to_path_buf());
    }
    Ok(())
}

fn is_supported_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some(
            "rs" | "toml"
                | "md"
                | "txt"
                | "json"
                | "yaml"
                | "yml"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "java"
                | "kt"
        )
    )
}

fn ensure_safe_relative_path(path: &Path) -> AgentOsResult<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentOsError::Validation(
            "planner file scope must be relative and stay inside workspace".to_string(),
        ));
    }
    Ok(())
}

fn score_candidate(task: &str, candidate: &InferredEdit) -> i32 {
    let mut score = 0;
    let task = task.to_ascii_lowercase();
    let file = candidate.file.to_string_lossy().to_ascii_lowercase();
    if file.contains("src/") || file.contains("src\\") {
        score -= 4;
    }
    if file.ends_with(".rs") {
        score -= 2;
    }
    for token in task
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|token| token.len() >= 3)
    {
        if file.contains(token) {
            score -= 1;
        }
    }
    score
}

fn word_number(value: &str) -> Option<u8> {
    match value.to_ascii_lowercase().as_str() {
        "zero" => Some(0),
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => None,
    }
}

fn number_word(value: u8) -> Option<&'static str> {
    match value {
        0 => Some("zero"),
        1 => Some("one"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InferredEdit {
    file: PathBuf,
    old: String,
    new: String,
}
