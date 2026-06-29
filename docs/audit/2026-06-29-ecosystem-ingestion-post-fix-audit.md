# Ecosystem Ingestion Post-Fix Audit

Date: 2026-06-29

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status after the fix remains dirty. Existing unrelated modified and
  untracked files were preserved.

## Implemented Fixes

### Current-Contract Fixes

1. Added shared ecosystem ABI/data contracts in `agent-os-sys` for sources,
   instruction documents, skills, commands, local stdio MCP servers/tools, and
   imported agent profiles.
2. Added kernel ecosystem projections and replayed events:
   `InstructionDocumentImported`, `SkillDefinitionImported`,
   `CommandDefinitionImported`, `McpServerRegistered`, `McpToolRegistered`, and
   `ImportedAgentProfileRegistered`.
3. Extended `ToolDescriptor` with `description`, `model_input_schema`,
   `runtime_input_policy`, and `driver_config` so runtime-visible schemas can be
   projected from kernel registrations.
4. Added native ecosystem tools:
   - `load_skill`
   - `read_skill_resource`
   - dynamic local stdio MCP tools named `mcp__server__tool`
5. Added Agent Thread runtime ecosystem import from workspace/global sources and
   model context projection for instructions, skills, commands, MCP tools, and
   imported agent profiles.
6. Added OpenAI/Anthropic model tool schema projection for `load_skill`,
   `read_skill_resource`, and permitted dynamic MCP tools.
7. Added CLI slash-command expansion for imported command templates using `$1`
   through `$9` and `$ARGUMENTS`; command execution remains audited through the
   normal runtime tools.
8. Updated architecture and conformance docs for ecosystem state, permission
   scopes, skill resource boundaries, MCP driver authorization, and new tests.

### Forward-Only Contract Choices

- OpenCode/Claude/.agents files are imported as Agent-OS ecosystem source
  records. They are not executed as compatibility shims.
- Unsupported shell interpolation in command templates is rejected.
- Local stdio is the only MCP transport in this fix. Remote MCP, OAuth, plugin
  runtimes, and custom JavaScript tools remain future ADR work.
- Duplicate skill names with different content fail closed. Duplicate names with
  identical content are coalesced as synchronized copies so global multi-tool
  skill installs do not block startup.
- Skill resource reads are canonicalized under the skill root and reject path
  escape.

## Changed Files

Primary files changed for this audit:

- `crates/agent-os-sys/src/ecosystem.rs`
- `crates/agent-os-sys/src/tools.rs`
- `crates/agent-os-kernel/src/ecosystem.rs`
- `crates/agent-os-kernel/src/events.rs`
- `crates/agent-os-kernel/src/state.rs`
- `crates/agent-os-kernel/src/tools.rs`
- `crates/agent-os-kernel/src/tools/driver/ecosystem.rs`
- `crates/agent-os-kernel/src/profile_seed/tools.rs`
- `crates/agent-os-kernel/src/profile_seed/permissions.rs`
- `crates/agent-os-thread/src/ecosystem.rs`
- `crates/agent-os-thread/src/ecosystem/scan.rs`
- `crates/agent-os-thread/src/ecosystem/mcp.rs`
- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/ecosystem_projection.rs`
- `crates/agent-os-thread/src/runtime/tool_policy.rs`
- `crates/agent-os-thread/src/model.rs`
- `crates/agent-os-thread/src/openai/client.rs`
- `crates/agent-os-thread/src/openai/prompt.rs`
- `crates/agent-os-thread/src/openai/tools.rs`
- `crates/agent-os-thread/src/openai/parser.rs`
- `crates/agent-os-thread/src/openai/messages.rs`
- `crates/agent-os-cli/src/chat.rs`
- `crates/agent-os-conformance/tests/integration/ecosystem_conformance.rs`
- `docs/10-kernel-design/kernel-data-model.md`
- `docs/10-kernel-design/permission-tool-evidence-model.md`
- `docs/20-implementation/conformance-and-quality.md`

## Validation Results

Passed during the fix:

```text
cargo fmt --all
cargo test -p agent-os-thread openai::tests::unit -- --nocapture
cargo test -p agent-os-conformance ecosystem_conformance -- --nocapture
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The focused conformance suite covers:

- project rule import and replay
- skill import, duplicate rejection, duplicate same-content coalescing
- command argument expansion and shell interpolation rejection
- imported agent profile projection
- `load_skill` and `read_skill_resource` permission and path-boundary behavior
- local stdio MCP `tools/list`, dynamic registration, `tools/call`, and
  permission denial
- runtime projection of imported ecosystem state into model context

## Remaining Gaps

- Dynamic projection from every kernel `ToolDescriptor` into every provider
  adapter is started for ecosystem/MCP tools but the older static OpenAI schema
  body still exists for core built-ins.
- Remote MCP, OAuth MCP, plugin runtimes, and custom JavaScript tool drivers are
  intentionally out of scope for this first ecosystem import.
- Ignored live LLM e2e for the combined rules + skill + MCP path should be added
  once provider credentials and spend policy are available.
