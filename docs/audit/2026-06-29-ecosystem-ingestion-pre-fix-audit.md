# Ecosystem Ingestion Pre-Fix Audit

Date: 2026-06-29

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status before this audit was dirty. Existing modified and untracked files are treated as user or prior-session work and must be preserved.

`git status --short` included existing changes across workspace manifests, CLI runtime files, conformance tests, kernel modules, thread OpenAI adapter files, docs, and several untracked v0.2/provider/permission audit artifacts. This fix must only add ecosystem-ingestion changes and avoid reverting unrelated local edits.

## Audit Scope

This audit covers the current gap between Agent-OS and the existing agent ecosystem represented by OpenCode:

- project and global instruction files such as `AGENTS.md` and `CLAUDE.md`
- reusable skills under Agent-OS/OpenCode/Claude/agents-compatible skill roots
- slash-command style prompt templates
- custom agent profile definitions
- local stdio MCP servers and their discovered tools

OpenCode reference paths inspected:

- `packages/opencode/src/session/instruction.ts`
- `packages/opencode/src/skill/index.ts`
- `packages/opencode/src/command/index.ts`
- `packages/opencode/src/mcp/index.ts`
- `packages/opencode/src/agent/agent.ts`
- `packages/web/src/content/docs/{rules,skills,commands,agents,mcp-servers,permissions,config}.mdx`

## Findings

### Current-Contract Gaps

1. Agent-OS already names MCP as a tool driver class, but there is no typed, replayable ecosystem registry for MCP servers, MCP tools, skills, commands, rules, or imported agent profiles.
2. Model-visible tool definitions are still hand-authored in `agent-os-thread`, duplicating kernel tool descriptors instead of projecting the kernel registry.
3. Skills and rules can only be simulated through prompt text today. They are not kernel-owned records, cannot be permission-scoped by skill identity, and cannot be replayed as imported ecosystem state.
4. There is no native `load_skill` or `read_skill_resource` tool, so a model cannot lazily load skill content while preserving audit and resource scope boundaries.
5. Commands and custom agents are not imported as typed contracts; they cannot participate in provider routing, profile policy, or conformance checks.

### Future-Roadmap Gaps

1. Remote MCP, OAuth, plugin runtimes, marketplace registry, and package signing are broader ecosystem capabilities and should remain outside this first fix.
2. Full dynamic projection of every tool descriptor into every provider adapter can be staged, but the public descriptor shape must be extended now so new ecosystem tools do not add another duplicated schema path.

## Validation Already Run

- `git rev-parse HEAD`
- `git rev-parse "HEAD^{tree}"`
- `git status --short`
- Targeted source inspection with `rg` and `Get-Content`

## Intended Fix Scope

- Add shared ecosystem ABI types in `agent-os-sys`.
- Add kernel ecosystem state, replayable registration events, discovery/import API, and native `load_skill` / `read_skill_resource` tools.
- Extend `ToolDescriptor` with model-facing metadata and runtime driver policy fields.
- Add local stdio MCP tool descriptor import as an audited first step, without remote/OAuth support.
- Add Agent-OS ecosystem discovery in `agent-os-thread` and include imported instruction summaries plus available skill metadata in the normal system prompt.
- Add focused unit and conformance tests for parsing, replay, permissions, skill loading, resource bounds, MCP descriptor registration, and runtime prompt projection.
- Update architecture docs and write a post-fix audit with validation results.

## Forward-Only Notes

- No legacy compatibility layer is introduced. OpenCode, Claude, and `.agents` files are treated as source formats imported into Agent-OS contracts.
- Unsupported ecosystem features must fail closed with validation errors rather than silently falling back to prompt-only behavior.
