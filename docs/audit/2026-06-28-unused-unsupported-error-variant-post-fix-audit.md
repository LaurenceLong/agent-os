# Unused Unsupported Error Variant Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-unused-unsupported-error-variant-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Removed the unused `AgentOsError::Unsupported` variant from
  `crates/agent-os-sys/src/core.rs`.
- Kept `unsupported_claims` fields and documentation unchanged because they
  describe verification findings, not operation support.

## Changed Files

- `crates/agent-os-sys/src/core.rs`
- `docs/audit/2026-06-28-unused-unsupported-error-variant-pre-fix-audit.md`
- `docs/audit/2026-06-28-unused-unsupported-error-variant-post-fix-audit.md`

## Forward-Only Notes

The shared error ABI no longer exposes an unused "unsupported operation"
category. Current-contract invalid inputs should use `Validation`, missing
resources should use `NotFound`, authorization failures should use
`PermissionDenied`, and resource/admission failures should use their specific
error variants.

## Validation Results

- `rg -n "AgentOsError::Unsupported|Unsupported\\(|unsupported operation|\\bUnsupported\\b" crates docs AGENTS.md distros --glob '!docs/audit/**' --glob '!target/**'`:
  only the unrelated `Unsupported claims` verification-language match remains.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract unused `Unsupported` error variant gaps remain for this
audit scope.
