# Syscall Unknown Op Error Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers error classification for unknown syscall operation names in
`crates/agent-os-kernel/src/syscall.rs`.

## Current-Contract Gap

`Kernel::handle_syscall` currently returns `AgentOsError::Unsupported` for any
operation string that does not match the current syscall table. In the
forward-only current contract, an unrecognized syscall name is invalid input,
not a supported-but-unavailable operation path.

## Future-Roadmap Gap

No syscall unknown-op error item is intentionally deferred in this scope.

## Intended Fix Scope

- Reclassify unknown syscall operation names as `AgentOsError::Validation`.
- Keep the public `AgentOsError::Unsupported` enum variant untouched in this
  pass because removing a shared ABI variant is broader than this focused
  dispatch correction.

## Validation Planned

- `rg` check for remaining non-ABI `AgentOsError::Unsupported` uses.
- Focused kernel unit test.
- Focused security conformance syscall test.
- `cargo fmt --check`
- `git diff --check`
