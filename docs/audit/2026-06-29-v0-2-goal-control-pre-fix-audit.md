# Agent-OS v0.2.0 Goal Control Pre-Fix Audit

Date: 2026-06-29

## Snapshot

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status before this fix:
  - Modified: `Cargo.lock`, `README.md`,
    `crates/agent-os-cli/Cargo.toml`, `crates/agent-os-cli/src/args.rs`,
    `crates/agent-os-cli/src/chat.rs`,
    `crates/agent-os-cli/src/chat/tests.rs`,
    `crates/agent-os-cli/src/main.rs`,
    `crates/agent-os-kernel/src/profile_seed/provider.rs`,
    `crates/agent-os-sys/src/provider.rs`,
    `crates/agent-os-thread/src/openai/client.rs`,
    `crates/agent-os-thread/src/openai/tests/live.rs`,
    `crates/agent-os-thread/src/openai/tests/unit.rs`,
    `docs/10-kernel-design/provider-system.md`,
    `docs/20-implementation/production-roadmap.md`
  - Untracked: `.env.example`,
    `crates/agent-os-cli/src/provider_config.rs`,
    `distros/providers.example.json`,
    `docs/audit/2026-06-29-env-config-split-post-fix-audit.md`,
    `docs/audit/2026-06-29-env-config-split-pre-fix-audit.md`

## Audit Scope

This audit covers the forward-only v0.2.0 control-plane closure:

- Replace the model-visible `set_objective` tool with Supervisor-only
  `set_goal`.
- Add execution-agent `accomplish_goal` for local goal completion.
- Move child creation-time work definition from `agent_control.payload.assignment`
  to `agent_control.payload.goal`.
- Track thread task binding as a goal with explicit status and revision.
- Complete active hooks when either `accomplish_goal` or successful
  `submit_final` closes the current thread.
- Convert `agent_control delete_session` and `purge_state` from v0.1
  append-only-store rejection into replayable logical lifecycle commands.
- Advance workspace crate version, ABI, and tool descriptor versions to
  `0.2.0`.

## Current-Contract Findings

1. The runtime and tool broker still expose `set_objective`, and the kernel
   implementation updates `Task.description` rather than a local agent goal.
2. `agent_control(action=start)` still requires `payload.assignment`, so child
   creation cannot atomically persist the canonical v0.2 child goal.
3. Thread task binding still uses `local_goal` without explicit goal status,
   revision, or accomplishment timestamp.
4. There is no model-visible `accomplish_goal` path, so execution agents cannot
   complete their local goal separately from `submit_final`.
5. Successful `submit_final` completes the thread but does not explicitly close
   active hooks for that thread, allowing stale hook injection state.
6. `delete_session` and `purge_state` are still rejected as append-only v0.1
   store actions instead of being recorded as applied replayable lifecycle
   commands.
7. Tool descriptors, OpenAI tool schemas, prompts, conformance tests, live test
   prompts, README text, and implementation docs still reference the v0.1
   `set_objective` and `assignment` surface.
8. ABI and crate versions remain at v0.1 values.

## Future-Roadmap Findings

- Distributed control-plane authorization, cross-host lifecycle management, and
  production operator UX remain beyond this v0.2.0 core closure.
- Physical compaction or historical erasure for purged sessions is intentionally
  out of scope; the store remains physically append-only while projections expose
  logical delete and purge state.
- Comprehensive provider-spend live e2e execution remains credential gated and
  may stay ignored by default.

## Validation Already Run

- `git status --short`
- `git cat-file -p HEAD`
- `rg` over `Cargo.toml`, `README.md`, `docs`, and `crates` for
  `set_objective`, `local_goal`, `assignment`, `append-only v0.1`,
  `delete_session`, `purge_state`, and related lifecycle symbols.

No compile, test, fmt, or clippy validation has run before the fix.

## Intended Fix Scope

- Update sys ABI/data types first, then kernel lifecycle/tool drivers, then
  OpenAI-compatible tool schemas/parser/prompt/runtime behavior.
- Update CLI/status formatting and tests to the renamed goal fields.
- Update conformance and live test expectations from rejection/v0.1 language to
  v0.2 success semantics.
- Update focused docs and README references in the touched public surface.
- Preserve unrelated dirty-tree changes and avoid reverting existing user work.
