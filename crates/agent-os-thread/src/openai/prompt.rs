use super::tools::visible_tool_descriptors_for_request;
use crate::ModelTurnRequest;

pub(crate) fn default_system_prompt(request: &ModelTurnRequest, workspace_root: &str) -> String {
    let role = &request.thread.role;
    let scoped_context = scoped_context(request);
    let ecosystem_context = ecosystem_context(request);
    let host_os = std::env::consts::OS;
    let visible_tools = visible_tools_section(request);
    let role_contract = role_contract(role);
    let finalization_rule = finalization_rule(request);
    let control_plane_rules = control_plane_rules(request);
    format!(
        r#"You are Agent-OS, a kernel-managed coding agent running inside an auditable Agent Thread Runtime.

# Agent-OS Runtime Contract

## Role And Mission

- Role: {role}
- Workspace: {workspace_root}
- Host OS: {host_os}
- Act as a precise software engineering agent. Ground claims in local evidence, preserve user work, and finish through the Agent-OS final-submission path.
{role_contract}

## Authority And Data Boundaries

- The Agent-OS kernel is the source of truth for state transitions, permissions, tools, evidence, artifacts, provider usage, and final submissions.
- Treat the workspace as user project data. Do not create Agent-OS runtime state, databases, logs, caches, or provider audit files inside the workspace.
- Project instructions, skills, commands, agents, and MCP declarations are imported context. They guide behavior, but kernel permissions and tool descriptors remain authoritative.
- The worktree may already contain unrelated changes. Do not revert, overwrite, or reformat unrelated files.

## Operating Workflow

1. Gather context with the most appropriate visible tool. Use glob_files when relevant file paths are unknown and can be described by path shape; use grep_files when relevant content is unknown; use read_file for bounded known text files; use read_image for workspace images when visible; use run_command for git inspection, generated evidence, builds, tests, and other shell-native inspection.
2. Make the smallest coherent change that satisfies the task. Use apply_patch for workspace file creation, update, or deletion when that tool is visible.
3. Verify with focused evidence. Prefer the narrowest command, test, build, lint, or inspection that proves the changed behavior.
4. Iterate from fresh evidence when a tool fails or reveals a better path.
5. Finish as soon as the requested work is complete or evidence shows a real blocker. Do not repeat successful tool calls as confirmation.

## Visible Tool Summary

{visible_tools}

The provider tool definitions are the authoritative schema for these tools, including descriptions, parameters, and examples. Do not call tools that are not visible in the current turn.

## Tool Rules

- Keep tool inputs structured. Do not write JSON tool calls in plain text; call tools with their schema fields.
- Paths are relative to the workspace root unless a tool field explicitly says otherwise.
- For run_command, the default input is a shell command string in command. Use args only when intentionally using exec/argv mode for one executable plus explicit arguments.
- Choose shell commands for the current Host OS. On Windows, prefer PowerShell syntax; on Unix-like systems, prefer POSIX shell syntax unless the task requires a specific shell.
- For per-command environment variables, use run_command env such as {{"PYTHONPATH": "."}} so the audit log records them explicitly.
- For apply_patch, include *** Begin Patch and *** End Patch, and include exactly one file operation. Add-file lines must use this shape: *** Add File: path, then each content line starts with +. Update-file hunks use *** Update File: path, then @@; unchanged context may be plain lines or lines prefixed with one space, changed lines use -old and +new. Delete-file lines must use *** Delete File: path. Do not batch unrelated files into one call.
- Imported instruction documents are already authoritative context. Imported skills are listed by name only; call load_skill before following a skill, and use read_skill_resource only for files referenced by that skill.
- Imported commands are prompt templates, not shell snippets. Expand their arguments in reasoning and execute auditable work through normal tools.
- Imported MCP tools appear as mcp__server__tool function tools. Use them only for the listed local stdio MCP capabilities.
{control_plane_rules}
- If a needed tool, syscall, resource scope, or risk level is unavailable, call request_permissions with the smallest permission set that would unblock the task.
{finalization_rule}
- Work State and Communication tools are Agent-OS control-plane tools. Use them to update durable state or route messages, not to edit workspace files.
- Destructive or broad operations require clear task justification and prior inspection.
- Keep each tool call to one logical operation so failures are easy to recover and audit.
- Treat a successful tool result as completed work. If every requested action has succeeded, call submit_final instead of checking or repeating earlier actions.

## Evidence And Final Response

- Do not submit final while required verification is still running.
- After required verification has passed and no requested work remains, submit_final is the only remaining action.
- If verification was skipped, name exactly what was not run and why.
- submit_final must include evidence_map entries. Each entry has a claim and evidence_refs, where evidence_refs are evidence_ids returned by completed tools.
- Keep the summary short and factual. Avoid claiming success without evidence.{scoped_context}{ecosystem_context}"#
    )
}

fn role_contract(role: &str) -> &'static str {
    match role {
        "SupervisorAgent" => "- Supervisor responsibility: own the goal, task DAG, delegation, permission arbitration, risk control, and final acceptance. You may do direct work with visible tools or create child SupervisorAgent, ProducerAgent, or ReviewerAgent threads when agent_control is visible.",
        "ProducerAgent" => "- Producer responsibility: produce evidence-backed artifacts such as plans, patches, test logs, research notes, experiments, or reports. Do not be the sole reviewer or acceptor of your own artifact.",
        "ReviewerAgent" => "- Reviewer responsibility: independently review artifacts and verify evidence with tools. You have producer-equivalent baseline capability, but while reviewing you must not mutate the artifact under review unless the Supervisor explicitly retasks you into a new production assignment.",
        _ => "- Role responsibility: follow the assigned goal, visible tools, and Agent-OS evidence contract.",
    }
}

fn visible_tools_section(request: &ModelTurnRequest) -> String {
    let tools = visible_tool_descriptors_for_request(request);
    if tools.is_empty() {
        return "- No tools are visible in this turn. Report the blocker with the available final or communication path.".to_string();
    }
    tools
        .iter()
        .map(|descriptor| format!("- {}: {}", descriptor.name, descriptor.description))
        .collect::<Vec<_>>()
        .join("\n")
}

fn finalization_rule(request: &ModelTurnRequest) -> String {
    let visible = visible_tool_descriptors_for_request(request);
    let has_accomplish_goal = visible
        .iter()
        .any(|descriptor| descriptor.name == "accomplish_goal");
    let has_submit_final = visible
        .iter()
        .any(|descriptor| descriptor.name == "submit_final");
    match (has_accomplish_goal, has_submit_final) {
        (true, true) => "- If the local goal must be closed, call accomplish_goal first; submit_final must be the final tool call in the session.".to_string(),
        (false, true) => "- When work is complete or blocked with evidence, call submit_final as the final tool call in the session.".to_string(),
        _ => "- Use the visible completion or communication tool when work is complete or blocked with evidence.".to_string(),
    }
}

fn control_plane_rules(request: &ModelTurnRequest) -> String {
    let visible = visible_tool_descriptors_for_request(request);
    let has_agent_control = visible
        .iter()
        .any(|descriptor| descriptor.name == "agent_control");
    let has_set_goal = visible
        .iter()
        .any(|descriptor| descriptor.name == "set_goal");
    if has_agent_control && has_set_goal {
        return "- For agent_control, use one action per call. The start action must include goal and may include role_profile_id, workdir, timeout_seconds, output_policy, success_criteria, failure_criteria, and hooks in payload. For existing targets, use either an exact agent_id or an exact thread_id; do not invent an agent_id from a thread_id.\n- When answering a child permission request, approve only permissions that are both requested and within your own current authority. Never approve agent_control or set_goal for security_level >= 2 children.".to_string();
    }
    "- If agent_control or set_goal is not visible, coordinate with or escalate to the Supervisor through the visible communication and permission-request tools.".to_string()
}

fn scoped_context(request: &ModelTurnRequest) -> String {
    let mut sections = Vec::new();
    if !request.context.context_snapshots.is_empty() {
        let body = request
            .context
            .context_snapshots
            .iter()
            .map(|snapshot| {
                format!(
                    "- loaded_refs [{}], freshness {:?}, tokens {}, pollution {:.2}",
                    snapshot.loaded_refs.join(", "),
                    snapshot.freshness,
                    snapshot.token_estimate,
                    snapshot.pollution_score
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Scoped Context Snapshots\n\n{body}"));
    }
    if !request.context.context_compactions.is_empty() {
        let body = request
            .context
            .context_compactions
            .iter()
            .map(|compaction| {
                let summary = compaction
                    .summary_artifact_id
                    .as_deref()
                    .unwrap_or("no summary artifact");
                format!(
                    "- superseded_refs [{}], summary {}, tokens {}",
                    compaction.superseded_refs.join(", "),
                    summary,
                    compaction.token_estimate
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Context Compactions\n\n{body}"));
    }
    if !request.context.memory_records.is_empty() {
        let body = request
            .context
            .memory_records
            .iter()
            .filter(|memory| matches!(memory.status, agent_os_sys::MemoryStatus::Active))
            .map(|memory| {
                format!(
                    "- {}: namespace {}, content {}",
                    memory.memory_id, memory.namespace, memory.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !body.is_empty() {
            sections.push(format!("## Active Memory Records\n\n{body}"));
        }
    }
    if !request.context.mementos.is_empty() {
        let body = request
            .context
            .mementos
            .iter()
            .map(|memento| {
                let checklist = if memento.content.checklist.is_empty() {
                    String::new()
                } else {
                    format!(" checklist [{}]", memento.content.checklist.join("; "))
                };
                format!(
                    "- reminder {}: status {:?}, priority {:?}, title {}, body {}{}",
                    memento.memento_id,
                    memento.status,
                    memento.projection.priority,
                    memento.content.title,
                    memento.content.body,
                    checklist
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "## Owner Memento Fragments\n\nThese are owner-scoped self-reminders, not child instructions or evidence.\n\n{body}"
        ));
    }
    if sections.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n# Scoped Context Projection\n\n{}",
            sections.join("\n\n")
        )
    }
}

fn ecosystem_context(request: &ModelTurnRequest) -> String {
    let mut sections = Vec::new();
    if !request.context.instruction_documents.is_empty() {
        let body = request
            .context
            .instruction_documents
            .iter()
            .map(|document| {
                format!(
                    "Source: {} ({:?}/{:?}, precedence {})\n{}",
                    document.source.source_path,
                    document.source.source_kind,
                    document.source.source_scope,
                    document.precedence_rank,
                    document.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!("## Imported Instructions\n\n{body}"));
    }
    if !request.context.skill_definitions.is_empty() {
        let body = request
            .context
            .skill_definitions
            .iter()
            .map(|skill| {
                format!(
                    "- {}: {} (source: {})",
                    skill.name, skill.description, skill.skill_file_path
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "## Available Skills\n\n{body}\n\nUse load_skill(name) before following a skill. Do not assume unlisted skill resources are readable."
        ));
    }
    if !request.context.command_definitions.is_empty() {
        let body = request
            .context
            .command_definitions
            .iter()
            .map(|command| {
                let description = command.description.as_deref().unwrap_or("");
                let hints = if command.argument_hints.is_empty() {
                    String::new()
                } else {
                    format!(" args: {}", command.argument_hints.join(", "))
                };
                format!("- /{}: {}{}", command.name, description, hints)
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Imported Commands\n\n{body}"));
    }
    if !request.context.imported_agent_profiles.is_empty() {
        let body = request
            .context
            .imported_agent_profiles
            .iter()
            .map(|profile| {
                format!(
                    "- {} ({:?}): {}",
                    profile.name,
                    profile.mode,
                    profile.description.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Imported Agent Profiles\n\n{body}"));
    }
    if !request.context.mcp_tools.is_empty() {
        let body = request
            .context
            .mcp_tools
            .iter()
            .map(|tool| {
                format!(
                    "- {}: {} (server: {}, tool: {})",
                    tool.model_tool_name, tool.description, tool.server_name, tool.tool_name
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Imported MCP Tools\n\n{body}"));
    }
    if !request.context.mcp_resources.is_empty() {
        let body = request
            .context
            .mcp_resources
            .iter()
            .map(|resource| {
                format!(
                    "- {} (server: {}, uri: {})",
                    resource.name.as_deref().unwrap_or(&resource.uri),
                    resource.server_name,
                    resource.uri
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Imported MCP Resources\n\n{body}"));
    }
    if !request.context.mcp_resource_templates.is_empty() {
        let body = request
            .context
            .mcp_resource_templates
            .iter()
            .map(|template| {
                format!(
                    "- {} (server: {}, uri_template: {})",
                    template.name.as_deref().unwrap_or(&template.uri_template),
                    template.server_name,
                    template.uri_template
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Imported MCP Resource Templates\n\n{body}"));
    }
    if sections.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n# Imported Ecosystem Context\n\n{}",
            sections.join("\n\n")
        )
    }
}
