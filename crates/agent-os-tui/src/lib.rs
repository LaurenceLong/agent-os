//! Terminal UI for Agent-OS.
//!
//! The TUI is a presentation client. It talks to Agent-OS through the app
//! protocol and never owns kernel or runtime authority.

mod app;
mod bottom_pane;
mod command_registry;
mod composer;
mod keymap;
mod overlay;
mod projection;
mod terminal;
mod timeline;

use agent_os_host::{default_state_db_for_workspace, StdioHostClient, StdioHostConfig};
use agent_os_sys::AgentOsResult;
use std::path::PathBuf;

pub use app::{TuiApp, TuiAppClient};
pub use bottom_pane::BottomPane;
pub use command_registry::{all_commands, CommandCategory, CommandDefinition, CommandTarget};
pub use composer::ComposerState;
pub use keymap::{default_keymap, KeyBinding};
pub use overlay::Overlay;
pub use projection::TuiProjection;
pub use timeline::timeline_lines;

#[derive(Debug, Clone, Default)]
pub struct TuiOptions {
    pub workspace: Option<PathBuf>,
    pub thread: Option<String>,
    pub resume: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub state_db: Option<PathBuf>,
    pub max_steps: Option<u32>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub no_alt_screen: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TuiExitReport {
    pub last_thread_id: Option<String>,
    pub submitted_turns: usize,
    pub final_status: Option<String>,
}

pub fn run_tui(options: TuiOptions) -> AgentOsResult<TuiExitReport> {
    let workspace = options
        .workspace
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let state_db = options
        .state_db
        .clone()
        .map(Ok)
        .unwrap_or_else(|| default_state_db_for_workspace(&workspace))?;
    let mut host_config = StdioHostConfig::state_db(state_db);
    host_config.model = options.model.clone();
    host_config.max_steps = options.max_steps;
    host_config.max_tokens = options.max_tokens;
    host_config.temperature = options.temperature.map(|value| value.to_string());
    let client = StdioHostClient::open(&host_config)?;
    let mut app = TuiApp::new(client, options);
    terminal::run(&mut app)
}
