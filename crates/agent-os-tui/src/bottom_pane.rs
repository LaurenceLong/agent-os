#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomPane {
    Status,
    Threads,
    Models,
    Permissions,
    Approvals,
    Processes,
}

impl BottomPane {
    pub fn title(self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Threads => "Threads",
            Self::Models => "Models",
            Self::Permissions => "Permissions",
            Self::Approvals => "Approvals",
            Self::Processes => "Processes",
        }
    }
}
