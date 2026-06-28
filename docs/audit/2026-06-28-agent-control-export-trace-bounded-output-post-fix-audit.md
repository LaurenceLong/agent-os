# Agent Control Export Trace Bounded Output Post-Fix Audit

## Git State

- HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- HEAD tree at post-fix validation: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status: dirty; this change was made inside the existing active roadmap/live-e2e worktree and preserves unrelated local changes.

## Implemented Fix

- Replaced `agent_control export_trace` full event payload output with bounded model-visible fields:
  - `event_count`
  - `event_types`
  - `first_event_id`
  - `last_event_id`
  - `preview_event_limit`
  - `preview_events`
  - `events_omitted`
- Removed the old full `output.events` contract from conformance expectations.
- Updated lifecycle conformance to assert the bounded trace contract and absence of the legacy full-events field.

## Changed Files

- `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`
- `crates/agent-os-conformance/tests/integration/lifecycle_conformance.rs`
- `docs/audit/2026-06-28-agent-control-export-trace-bounded-output-pre-fix-audit.md`

## Compatibility Notes

- Forward-only contract: no fallback full-event array and no backward-compatible alias.
- The model-visible trace response is intentionally summary-first to keep live runtime context bounded.
- Durable event export remains available through the store/export path; this fix only changes the `agent_control export_trace` tool response.

## Validation

- `cargo test -p agent-os-conformance --test integration_tests agent_control_lifecycle_actions_update_state_and_trace` passed.
- `cargo test -p agent-os-conformance --test integration_tests goal_driven_runtime_integration_covers_tools_and_agent_control_actions` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo fmt --check` passed.
- `git diff --check` passed with only existing CRLF normalization warnings.

## Remaining Gaps

- Durable trace handles and pagination remain future roadmap work.
