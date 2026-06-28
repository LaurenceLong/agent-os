# Agent Thread Op Naming Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-agent-thread-op-naming-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Renamed the public Agent Thread op constructor from `mock_turn_start_op` to
  `turn_start_op`.
- Updated the public `agent-os-thread` re-export to expose only
  `turn_start_op`.
- Removed the mock-oriented unknown-op rejection text from
  `AgentThreadHandle`.
- Updated the conformance lifecycle test import, call site, and test name to
  describe `AgentThreadHandle` behavior rather than a mock runtime.
- No compatibility alias was added for the old public name.

## Changed Files

- `crates/agent-os-thread/src/ops.rs`
- `crates/agent-os-thread/src/lib.rs`
- `crates/agent-os-thread/src/handle.rs`
- `crates/agent-os-conformance/tests/integration/lifecycle_conformance.rs`
- `docs/audit/2026-06-28-agent-thread-op-naming-pre-fix-audit.md`
- `docs/audit/2026-06-28-agent-thread-op-naming-post-fix-audit.md`

## Forward-Only Notes

The public Agent Thread API now exposes a canonical turn-start op constructor.
The old mock-oriented name was removed instead of retained as an alias.

## Validation Results

- `rg -n "mock_turn_start_op|mock runtime op|mock_runtime_admits" crates docs AGENTS.md distros --glob '!docs/audit/**'`: no matches.
- `cargo test -p agent-os-conformance --test integration_tests`: passed, 66
  tests.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract Agent Thread op naming gaps remain for this audit scope.
