#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Keymap,
    Context,
    Events,
    Replay,
    Evidence,
    Artifacts,
    Diff,
    Debug,
    Tools,
    Mcp,
    Usage,
    Provider,
}

impl Overlay {
    pub fn title(self) -> &'static str {
        match self {
            Self::Help => "Help",
            Self::Keymap => "Keymap",
            Self::Context => "Context",
            Self::Events => "Events",
            Self::Replay => "Replay",
            Self::Evidence => "Evidence",
            Self::Artifacts => "Artifacts",
            Self::Diff => "Diff",
            Self::Debug => "Debug",
            Self::Tools => "Tools",
            Self::Mcp => "MCP",
            Self::Usage => "Usage",
            Self::Provider => "Provider",
        }
    }
}
