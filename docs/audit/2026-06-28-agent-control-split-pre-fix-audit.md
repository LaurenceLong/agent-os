# Agent Control Module Split Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status: clean.

## Audit Scope

This is the next iteration of the maintained `audit -> document -> fix ->
document` workflow. The prior iterations are recorded in:

- `2026-06-28-pre-fix-design-audit.md`
- `2026-06-28-post-fix-audit.md`
- `2026-06-28-testing-strategy-pre-update.md`
- `2026-06-28-testing-strategy-post-fix-audit.md`
- `2026-06-28-live-llm-e2e-policy-pre-fix-audit.md`
- `2026-06-28-live-llm-e2e-policy-post-fix-audit.md`

This iteration reviews module-size debt flagged as a follow-up risk in the
design-audit post-fix document and decides whether it is safe to resolve now.

## Verification At Audit Start

- `cargo build --workspace`: passed.
- `cargo test --workspace`: passed.
  - 28 non-ignored tests passed across all crates.
  - 10 live LLM e2e tests remained ignored because they require real provider
    credentials.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.

## Findings

1. `crates/agent-os-kernel/src/tools/driver/agent_control.rs` is 565 lines.
   The repository rules in `AGENTS.md` require files over 600 lines to be
   split before adding substantial new behavior, and treat files over 1000
   lines as architectural debt. The prior design-audit post-fix document
   already named this file as a follow-up risk because the stateful
   `agent_control` lifecycle work pushed it close to the threshold.

2. The file currently mixes several distinct ownership areas:
   - Top-level action dispatch (`run_agent_control`).
   - Stateful lifecycle action implementation (`apply_lifecycle_action`,
     `output_for_target`, `export_trace_for_target`, `set_timeout_budget`,
     `terminate_target`).
   - Risk enforcement and action parsing (`require_agent_control_action_risk`,
     `parse_agent_control_action`).
   - Target resolution and start/hook configuration
     (`resolve_agent_control_target`, `configure_start_hooks`,
     `configure_agent_hook`, `agent_control_workspace_roots`).
   - Durable command recording and lookup helpers
     (`record_agent_control_command`, `agent_hooks_for`).

   `AGENTS.md` asks production modules to be split by ownership, naming tool
   drivers and parser/driver families as examples of good split boundaries.

3. No current-contract defect was found in `agent_control` behavior. The
   lifecycle actions, privileged risk enforcement, and append-only-store
   rejection paths are covered by:
   - `agent_control_lifecycle_actions_update_state_and_trace`
   - `privileged_agent_control_actions_require_privileged_risk`
   - `goal_driven_runtime_integration_covers_tools_and_agent_control_actions`
   - `goal_driven_runtime_integration_covers_privileged_agent_control_rejections`
   - live LLM e2e coverage under `agent-os-thread/src/openai/tests/live.rs`.

## Current-Contract Gaps Addressed This Turn

- Split `agent_control.rs` by ownership so the file stays comfortably under
  the 600-line threshold and the lifecycle, parsing, target-resolution, hook,
  and command-recording responsibilities live in focused modules.

## Future-Roadmap Gaps (Not Addressed This Turn)

The roadmap-level items recorded in the prior post-fix audit remain open and
are intentionally not release blockers for this iteration:

- Scheduler admission wiring to budgets, provider slots, human attention, and
  resource pressure.
- Runtime checkpointing at every documented yield boundary.
- Recovery reconciliation of orphan running tools and workspace diffs.
- Comprehensive final-verification claim parsing and stale-evidence coverage.
- Typed context projection, memory write policy, and compaction provenance.
- Full store-family trait coverage from the design.
- Provider credential resolution, quota policy, retry policy, transforms, and
  provider-slot admission.
- The official software-engineering distribution package, manifest, and policy
  pack.

## Intended Fix Scope

- Move the stateful lifecycle helpers into a new
  `tools/driver/agent_control/lifecycle.rs` module.
- Move action parsing and risk enforcement into
  `tools/driver/agent_control/action.rs`.
- Move target resolution, start/hook configuration, command recording, and
  hook lookup into `tools/driver/agent_control/hooks.rs`,
  `tools/driver/agent_control/command.rs`, and keep the dispatch entry point
  in `agent_control.rs` (or a thin `mod.rs`).
- Preserve the existing public surface: the only crate-internal entry point is
  `run_agent_control`, called from `tools/driver.rs`. No function signature,
  event payload, tool name, tool schema, risk mapping, or persisted shape may
  change.
- Keep all existing tests green without modification. If a test references an
  internal path that moves, update only the path, not the assertions.

## Validation Planned

- `cargo fmt`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
