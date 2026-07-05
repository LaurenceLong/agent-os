#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Conversation,
    Thread,
    Model,
    Permissions,
    Inspection,
    Goal,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTarget {
    AppRequest,
    UiOnly,
    ProjectionOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDefinition {
    pub id: &'static str,
    pub slash: &'static str,
    pub aliases: &'static [&'static str],
    pub title: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub inline_args: bool,
    pub running_turn_allowed: bool,
    pub side_allowed: bool,
    pub target: CommandTarget,
}

pub fn all_commands() -> &'static [CommandDefinition] {
    COMMANDS
}

pub fn command_by_slash(input: &str) -> Option<&'static CommandDefinition> {
    let name = input.trim_start_matches('/').split_whitespace().next()?;
    COMMANDS
        .iter()
        .find(|command| command.slash == name || command.aliases.contains(&name))
}

const COMMANDS: &[CommandDefinition] = &[
    command(
        "new",
        CommandCategory::Conversation,
        CommandTarget::UiOnly,
        false,
        false,
        "New thread",
        "Clear the local composer and prepare a new thread.",
    ),
    command(
        "run",
        CommandCategory::Conversation,
        CommandTarget::AppRequest,
        true,
        true,
        "Run prompt",
        "Submit the composer or inline prompt to the LLM.",
    ),
    command(
        "interrupt",
        CommandCategory::Conversation,
        CommandTarget::AppRequest,
        false,
        true,
        "Interrupt turn",
        "Interrupt the running turn.",
    ),
    command(
        "steer",
        CommandCategory::Conversation,
        CommandTarget::AppRequest,
        true,
        true,
        "Steer turn",
        "Send steering input to a running turn.",
    ),
    command(
        "status",
        CommandCategory::Conversation,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Status",
        "Open the current status panel.",
    ),
    command(
        "raw",
        CommandCategory::Conversation,
        CommandTarget::UiOnly,
        true,
        true,
        "Raw output",
        "Toggle raw timeline and projection display.",
    ),
    command(
        "copy",
        CommandCategory::Conversation,
        CommandTarget::UiOnly,
        false,
        true,
        "Copy",
        "Copy the selected or latest agent item.",
    ),
    command(
        "clear",
        CommandCategory::Conversation,
        CommandTarget::UiOnly,
        false,
        true,
        "Clear UI",
        "Clear local scrollback without changing kernel state.",
    ),
    CommandDefinition {
        aliases: &["quit"],
        ..command(
            "exit",
            CommandCategory::Conversation,
            CommandTarget::UiOnly,
            false,
            true,
            "Exit",
            "Exit the TUI.",
        )
    },
    CommandDefinition {
        aliases: &["sessions"],
        ..command(
            "threads",
            CommandCategory::Thread,
            CommandTarget::AppRequest,
            false,
            true,
            "Threads",
            "Open the thread picker.",
        )
    },
    command(
        "resume",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        true,
        false,
        "Resume",
        "Resume a thread or runtime job.",
    ),
    command(
        "fork",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        true,
        false,
        "Fork",
        "Fork the current thread.",
    ),
    command(
        "rollback",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        true,
        false,
        "Rollback",
        "Rollback the current thread.",
    ),
    command(
        "rename",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        true,
        true,
        "Rename",
        "Rename the current thread.",
    ),
    command(
        "archive",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        false,
        false,
        "Archive",
        "Archive the current thread.",
    ),
    command(
        "delete",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        false,
        false,
        "Delete",
        "Delete the current thread after confirmation.",
    ),
    command(
        "compact",
        CommandCategory::Thread,
        CommandTarget::AppRequest,
        false,
        false,
        "Compact",
        "Request context compaction.",
    ),
    CommandDefinition {
        aliases: &["models"],
        ..command(
            "model",
            CommandCategory::Model,
            CommandTarget::UiOnly,
            true,
            false,
            "Model",
            "Open or update the model selection.",
        )
    },
    CommandDefinition {
        aliases: &["providers"],
        ..command(
            "provider",
            CommandCategory::Model,
            CommandTarget::ProjectionOnly,
            false,
            true,
            "Provider",
            "Inspect provider capabilities.",
        )
    },
    command(
        "profile",
        CommandCategory::Model,
        CommandTarget::UiOnly,
        true,
        false,
        "Profile",
        "Inspect or stage a profile selection.",
    ),
    command(
        "usage",
        CommandCategory::Model,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Usage",
        "Open provider usage and stats.",
    ),
    command(
        "permissions",
        CommandCategory::Permissions,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Permissions",
        "Open permission profile and risk status.",
    ),
    command(
        "approve",
        CommandCategory::Permissions,
        CommandTarget::AppRequest,
        false,
        true,
        "Approvals",
        "Open pending approvals.",
    ),
    command(
        "tools",
        CommandCategory::Permissions,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Tools",
        "Inspect model-visible tools.",
    ),
    command(
        "mcp",
        CommandCategory::Permissions,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "MCP",
        "Inspect MCP tools and resources.",
    ),
    CommandDefinition {
        aliases: &["ps"],
        ..command(
            "processes",
            CommandCategory::Permissions,
            CommandTarget::ProjectionOnly,
            false,
            true,
            "Processes",
            "Inspect process sessions.",
        )
    },
    command(
        "stop",
        CommandCategory::Permissions,
        CommandTarget::AppRequest,
        true,
        true,
        "Stop process",
        "Stop a process session.",
    ),
    command(
        "kill",
        CommandCategory::Permissions,
        CommandTarget::AppRequest,
        true,
        true,
        "Kill process",
        "Kill a process session.",
    ),
    command(
        "context",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Context",
        "Open model context projection.",
    ),
    command(
        "events",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Events",
        "Open event timeline.",
    ),
    command(
        "replay",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Replay",
        "Open replay transcript.",
    ),
    command(
        "evidence",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Evidence",
        "Inspect evidence index.",
    ),
    command(
        "artifacts",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Artifacts",
        "Inspect artifact index.",
    ),
    command(
        "diff",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Diff",
        "Open workspace diff view.",
    ),
    command(
        "debug",
        CommandCategory::Inspection,
        CommandTarget::ProjectionOnly,
        false,
        true,
        "Debug",
        "Open debug projection.",
    ),
    command(
        "goal",
        CommandCategory::Goal,
        CommandTarget::UiOnly,
        true,
        true,
        "Goal",
        "Inspect or stage goal intent.",
    ),
    command(
        "plan",
        CommandCategory::Goal,
        CommandTarget::AppRequest,
        true,
        true,
        "Plan",
        "Submit a planning-style message.",
    ),
    CommandDefinition {
        aliases: &["scratch"],
        ..command(
            "side",
            CommandCategory::Goal,
            CommandTarget::UiOnly,
            true,
            true,
            "Side",
            "Open a local side composer.",
        )
    },
    command(
        "help",
        CommandCategory::Help,
        CommandTarget::UiOnly,
        false,
        true,
        "Help",
        "Open generated command help.",
    ),
    command(
        "keymap",
        CommandCategory::Help,
        CommandTarget::UiOnly,
        false,
        true,
        "Keymap",
        "Open keymap help.",
    ),
    command(
        "theme",
        CommandCategory::Help,
        CommandTarget::UiOnly,
        false,
        true,
        "Theme",
        "Open theme picker.",
    ),
    command(
        "editor",
        CommandCategory::Help,
        CommandTarget::UiOnly,
        false,
        true,
        "Editor",
        "Edit the composer in an external editor.",
    ),
];

const fn command(
    slash: &'static str,
    category: CommandCategory,
    target: CommandTarget,
    inline_args: bool,
    running_turn_allowed: bool,
    title: &'static str,
    description: &'static str,
) -> CommandDefinition {
    CommandDefinition {
        id: slash,
        slash,
        aliases: &[],
        title,
        description,
        category,
        inline_args,
        running_turn_allowed,
        side_allowed: false,
        target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_full_tui_surface() {
        let names = all_commands()
            .iter()
            .map(|command| command.slash)
            .collect::<std::collections::BTreeSet<_>>();

        for required in [
            "run",
            "interrupt",
            "threads",
            "resume",
            "fork",
            "rollback",
            "model",
            "permissions",
            "tools",
            "context",
            "evidence",
            "goal",
            "plan",
            "help",
        ] {
            assert!(names.contains(required), "missing /{required}");
        }
    }

    #[test]
    fn registry_resolves_aliases() {
        assert_eq!(command_by_slash("/quit").unwrap().slash, "exit");
        assert_eq!(command_by_slash("/sessions").unwrap().slash, "threads");
        assert_eq!(command_by_slash("/ps").unwrap().slash, "processes");
    }
}
