use crate::support::{default_state_db_for_workspace, StdioHostAppClient, StdioHostConfig};
use agent_os_sys::{AgentOsResult, AppRequest, StatsQuery};
use serde_json::{json, Value};

pub(crate) fn run_demo() -> AgentOsResult<Value> {
    let workspace = std::env::current_dir().map_err(|error| {
        agent_os_sys::AgentOsError::Validation(format!("resolve current directory: {error}"))
    })?;
    let state_db = default_state_db_for_workspace(&workspace)?;
    let mut client = StdioHostAppClient::open(&StdioHostConfig::state_db(&state_db))?;
    client.request(AppRequest::Initialize)?;
    let started = client.request(AppRequest::ThreadStart {
        goal: "Demo Agent-OS lifecycle".to_string(),
        workspace: Some(workspace.to_string_lossy().to_string()),
    })?;
    let listed = client.request(AppRequest::ThreadList {
        archived: Some(false),
    })?;
    let stats = client.request(AppRequest::StatsRead {
        query: StatsQuery::default(),
    })?;

    Ok(json!({
        "thread": started["thread"],
        "threads": listed["threads"],
        "stats": stats["snapshot"],
        "state_db": state_db.to_string_lossy(),
        "transport": "app-server-jsonl",
    }))
}
