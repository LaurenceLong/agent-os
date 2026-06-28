# Agent Control Rejection Error Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-agent-control-rejection-error-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Changed append-only-store `delete_session` and `purge_state` rejections from
  `AgentOsError::Unsupported` to `AgentOsError::Validation`.
- Changed impossible lifecycle-module dispatch from
  `AgentOsError::Unsupported` to `AgentOsError::Validation`.
- Updated the goal-driven `agent_control` rejection conformance test to expect
  `AppendOnlyStoreRejected` validation semantics.
- Updated the live LLM e2e rejection assertion to expect the same validation
  semantics.

## Changed Files

- `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/mod.rs`
- `crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs`
- `crates/agent-os-thread/src/openai/tests/live.rs`
- `docs/audit/2026-06-28-agent-control-rejection-error-pre-fix-audit.md`
- `docs/audit/2026-06-28-agent-control-rejection-error-post-fix-audit.md`

## Forward-Only Notes

The append-only-store behavior is now expressed as an intentional current
contract rejection, not as an unsupported feature path.

## Validation Results

- `rg -n "Unsupported|unsupported lifecycle action|not available in the append-only|AppendOnlyStoreRejected|append-only v0.1 store" crates/agent-os-kernel/src/tools/driver/agent_control crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs crates/agent-os-thread/src/openai/tests/live.rs`:
  no `Unsupported` matches remain in the `agent_control` paths; expected
  validation-rejection matches remain.
- `cargo test -p agent-os-conformance --test integration_tests goal_driven_runtime_integration_covers_privileged_agent_control_rejections`:
  passed.
- `cargo test -p agent-os-thread --lib --no-run`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract `agent_control` rejection-error gaps remain for this audit
scope.
