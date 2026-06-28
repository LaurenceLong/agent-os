use agent_os_sys::*;
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub(crate) fn append_jsonl(path: &Path, entry: &Value) -> AgentOsResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AgentOsError::Validation(format!("create audit dir: {error}")))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| AgentOsError::Validation(format!("open audit log: {error}")))?;
    writeln!(file, "{}", serde_json::to_string(entry)?)
        .map_err(|error| AgentOsError::Validation(format!("write audit log: {error}")))?;
    Ok(())
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
