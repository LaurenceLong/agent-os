# Live E2E Rejection Naming Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-live-e2e-rejection-naming-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Renamed live `agent_control delete_session` / `purge_state` e2e tests from
  unsupported-action terminology to append-only-store rejection terminology.
- Renamed the live test helpers, temporary directory prefix, audit log file
  names, JSON log event types, printed log label, task titles, task
  descriptions, and target label to use rejection terminology.
- Replaced live fixture `local_goal: "placeholder"` values with concrete live
  scenario goals.
- Updated `2026-06-28-live-llm-e2e-policy-post-fix-audit.md` so the live
  negative scenario is described as append-only-store rejection coverage.
- Kept the low-level `AgentOsError::Unsupported` assertion unchanged because it
  is the current error variant used by the rejected append-only-store actions.
- No aliases were added for the old test names.

## Changed Files

- `crates/agent-os-thread/src/openai/tests/live.rs`
- `docs/audit/2026-06-28-live-llm-e2e-policy-post-fix-audit.md`
- `docs/audit/2026-06-28-live-e2e-rejection-naming-pre-fix-audit.md`
- `docs/audit/2026-06-28-live-e2e-rejection-naming-post-fix-audit.md`

## Forward-Only Notes

The live e2e surface now names the negative `agent_control` scenario as a
current fail-closed rejection path rather than as a temporary unsupported
implementation gap.

## Validation Results

- `rg -n "unsupported|placeholder|agent_control_unsupported|goal-agent-control-unsupported|live_goal_agent_control_unsupported" crates/agent-os-thread/src/openai/tests/live.rs docs/audit/2026-06-28-live-llm-e2e-policy-post-fix-audit.md`: no matches.
- `cargo test -p agent-os-thread --lib --no-run`: passed.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract live e2e rejection naming gaps remain for this audit scope.
