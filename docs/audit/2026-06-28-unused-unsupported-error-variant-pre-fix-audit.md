# Unused Unsupported Error Variant Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers the public `AgentOsError::Unsupported` variant in
`crates/agent-os-sys/src/core.rs`.

## Current-Contract Gap

Forward-only error classification now treats unknown or invalid current-contract
inputs as `Validation`, missing resources as `NotFound`, permission boundaries
as `PermissionDenied`, and deterministic resource/admission failures as their
specific variants.

After the `agent_control` and syscall dispatch corrections, no production or
test code constructs `AgentOsError::Unsupported`. Keeping the unused variant
preserves a misleading "unsupported operation" category in the shared ABI and
invites future fallback-style semantics.

## Future-Roadmap Gap

No error-variant cleanup item is intentionally deferred in this scope.

## Intended Fix Scope

- Remove `AgentOsError::Unsupported`.
- Do not rename unrelated `unsupported_claims` fields or documentation; those
  describe verification findings, not operation support.

## Validation Planned

- `rg` check for `AgentOsError::Unsupported`, `Unsupported(`, and
  `unsupported operation`.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
