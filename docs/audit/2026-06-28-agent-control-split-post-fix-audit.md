# Agent Control Module Split Post-Fix Audit

Date: 2026-06-28

Correction note: the roadmap-level remaining-gap status in this document is
superseded by `2026-06-28-roadmap-gaps-post-fix-audit.md`, which records those
items as closed for the current forward-only contract. The compatibility
language in this historical audit is superseded by
`2026-06-28-agents-forward-only-rules-post-fix-audit.md`.

## Code Identity

- Base Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Base Git tree: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status after the split: `agent_control.rs` deleted, new
  `agent_control/` module directory added, pre-fix and post-fix audit records
  added under `docs/audit/`.

## Implemented Fix

Split the single 565-line `agent_control.rs` tool driver into a directory
module organized by ownership, as required by `AGENTS.md` for files
approaching the 600-line split-before-growth threshold. The split is a pure
internal refactor: no public API, tool name, tool schema, risk mapping, event
payload, or persisted shape changed.

New module layout under
`crates/agent-os-kernel/src/tools/driver/agent_control/`:

- `mod.rs` (185 lines): top-level `run_agent_control` dispatch and the
  `AgentControlActionResult` struct.
- `lifecycle.rs` (192 lines): stateful lifecycle actions
  (`apply_lifecycle_action`, `output_for_target`,
  `export_trace_for_target`, `set_timeout_budget`, `terminate_target`).
- `action.rs` (52 lines): action-name parsing and kernel-side risk gating
  (`parse_agent_control_action`, `require_agent_control_action_risk`).
- `target.rs` (69 lines): target thread resolution and child workspace-root
  resolution (`resolve_agent_control_target`,
  `agent_control_workspace_roots`).
- `hooks.rs` (86 lines): agent hook configuration and lookup
  (`configure_start_hooks`, `configure_agent_hook`, `agent_hooks_for`).
- `command.rs` (42 lines): durable `AgentControlCommand` recording
  (`record_agent_control_command`).

Every previously private function that crossed a module boundary is now
`pub(super)` so visibility is unchanged outside the `agent_control` module.
The crate-internal entry point `run_agent_control` remains the single symbol
re-exported to `tools/driver.rs`, whose `mod agent_control;` declaration now
resolves to `agent_control/mod.rs` automatically.

## Changed Files

- Deleted: `crates/agent-os-kernel/src/tools/driver/agent_control.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/mod.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/action.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/command.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/hooks.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`.
- Added: `crates/agent-os-kernel/src/tools/driver/agent_control/target.rs`.
- Added: `docs/audit/2026-06-28-agent-control-split-pre-fix-audit.md`.
- Added: `docs/audit/2026-06-28-agent-control-split-post-fix-audit.md`.

No production file outside the `agent_control` module was modified.
`tools/driver.rs` keeps its existing `mod agent_control;` declaration
unchanged.

## Validation

- `cargo fmt`: passed (no formatting changes required).
- `cargo build --workspace`: passed.
- `cargo test --workspace`: passed.
  - Identical to the pre-fix baseline: 28 non-ignored tests passed and 10 live
    LLM e2e tests remained ignored because they require real provider
    credentials. No test source was modified.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.

## Historical Notes

- No SQLite schema migration was added or required by this split.
- No persisted event shape was removed or changed by this split.
- The `agent_control` tool name, input schema, output JSON shape, risk-level
  requirements, and command-recording behavior were unchanged by this split.
- Existing tests for stateful lifecycle actions, privileged risk enforcement,
  and append-only-store rejection paths passed without modification:
  - `agent_control_lifecycle_actions_update_state_and_trace`
  - `privileged_agent_control_actions_require_privileged_risk`
  - `agent_control_starts_child_and_records_hook_state`
  - `goal_driven_runtime_integration_covers_tools_and_agent_control_actions`
  - `goal_driven_runtime_integration_covers_privileged_agent_control_rejections`
  - `submit_final_lifecycle_tool_records_final_submission`
  - `tool_broker_integration_runs_all_model_visible_tool_families`

## Remaining Gaps

The follow-up risk recorded in the prior design-audit post-fix document is
resolved for `agent_control`. The roadmap-level gaps listed below were not
addressed by this split, and their current status is superseded by
`2026-06-28-roadmap-gaps-post-fix-audit.md`:

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

The next-largest production module is `profile_seed/tools.rs` at 526 lines,
which is below the 600-line threshold and does not require immediate action.
