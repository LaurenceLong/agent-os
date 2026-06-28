# Software Roles Module Size Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers the software role orchestration module at
`crates/agent-os-thread/src/software/roles.rs`.

## Current-Contract Gap

After replacing production scripted model execution with explicit kernel/tool
operations, `roles.rs` reached 615 lines. Project rules require splitting
production files over 600 lines before adding substantial new behavior.

The new deterministic tool-role workflow is a separate responsibility from
role-level orchestration, so keeping it in `roles.rs` creates an avoidable
ownership boundary violation.

## Future-Roadmap Gap

This pass does not split every production module above the preferred 400-line
guideline. It addresses the newly introduced hard-limit breach in the file
touched by the current software pipeline change.

## Intended Fix Scope

- Move deterministic tool-role execution into a dedicated software module.
- Keep role orchestration in `roles.rs`.
- Preserve the clean forward-only software distribution behavior with no
  scripted production runtime path.

## Validation Planned

- Confirm `roles.rs` is under 600 lines.
- `cargo fmt --check`
- Focused software distribution tests.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
