# Scripted Model Public Surface Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers the public `agent-os-thread` crate surface for deterministic
model execution used by tests.

## Current-Contract Gap

`agent-os-thread` still exposes `ScriptedModelClient` and `ScriptedStep` from
the production crate root. After the software distribution stopped using a
scripted production runtime path, keeping that deterministic test client in the
public runtime surface makes a test fixture look like a supported model
adapter.

The clean forward contract should expose real runtime interfaces and provider
adapters only. Deterministic model fixtures belong in unit and conformance test
code.

## Future-Roadmap Gap

This pass does not replace deterministic integration tests with live LLM tests.
Live LLM e2e tests remain separate and ignored by default when credentials are
missing.

## Intended Fix Scope

- Remove `ScriptedModelClient` and `ScriptedStep` from the public
  `agent-os-thread` crate surface.
- Move deterministic model behavior into test-only fixtures.
- Preserve runtime unit and conformance coverage without production scripted
  runtime APIs.

## Validation Planned

- `rg` check that `ScriptedModelClient`, `ScriptedStep`, and `scripted` are
  absent from production crate/docs paths outside audit records.
- Focused runtime and conformance tests that previously used the scripted
  client.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
