# Syscall Unknown Op Error Post-Fix Audit

Date: 2026-06-28

Correction note: the public `AgentOsError::Unsupported` enum variant mentioned
below was later removed by
`2026-06-28-unused-unsupported-error-variant-post-fix-audit.md`.

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-syscall-unknown-op-error-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Changed unknown syscall operation names from `AgentOsError::Unsupported` to
  `AgentOsError::Validation`.
- Updated the error text to `unknown syscall op {op}`.
- Left the public `AgentOsError::Unsupported` enum variant in place for this
  focused dispatch correction. That follow-up cleanup is recorded in
  `2026-06-28-unused-unsupported-error-variant-post-fix-audit.md`.

## Changed Files

- `crates/agent-os-kernel/src/syscall.rs`
- `docs/audit/2026-06-28-syscall-unknown-op-error-pre-fix-audit.md`
- `docs/audit/2026-06-28-syscall-unknown-op-error-post-fix-audit.md`

## Forward-Only Notes

Unknown syscall names are now treated as invalid current-contract input instead
of as an unsupported operation path.

## Validation Results

- `rg -n "AgentOsError::Unsupported|Unsupported\\(" crates --glob '!target/**'`:
  at the time of this audit, only the public `AgentOsError::Unsupported` enum
  variant remained. That later became no matches after
  `2026-06-28-unused-unsupported-error-variant-post-fix-audit.md`.
- `cargo test -p agent-os-kernel --test kernel_unit syscall_without_capability_is_rejected`:
  passed.
- `cargo test -p agent-os-conformance --test integration_tests syscall_without_capability_is_rejected`:
  passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract syscall unknown-op error gaps remain for this audit scope.
