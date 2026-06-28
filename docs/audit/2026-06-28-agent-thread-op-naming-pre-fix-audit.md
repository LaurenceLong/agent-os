# Agent Thread Op Naming Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only build
  changes and audit records.

## Scope

This audit covers the public Agent Thread op constructor exposed by
`agent-os-thread`.

## Current-Contract Gap

- `crates/agent-os-thread/src/ops.rs` exposes `mock_turn_start_op`.
- `crates/agent-os-thread/src/lib.rs` re-exports `mock_turn_start_op` as part of
  the public crate surface.
- `crates/agent-os-thread/src/handle.rs` reports unknown submitted ops as
  "unsupported mock runtime op".
- The only current caller is a conformance integration test.

This conflicts with the forward-only rule that public surfaces should represent
the canonical current contract rather than mock or compatibility concepts.

## Intended Fix Scope

- Rename the public constructor to `turn_start_op`.
- Update the conformance test import and call site.
- Rename the affected test from mock-runtime language to AgentThreadHandle
  language.
- Change the unknown-op rejection reason to "unknown runtime op".
- Do not add a compatibility alias for the old `mock_turn_start_op` name.

## Validation Planned

- `rg` check that `mock_turn_start_op` and "mock runtime op" no longer appear.
- `cargo test -p agent-os-conformance --test integration_tests`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
