# Software Pipeline Scripted Runtime Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers production software-engineering distribution execution in
`crates/agent-os-thread/src/software/roles.rs`.

## Current-Contract Gap

The production `SoftwareEngineeringPipeline` runs Explorer, Coder, and Tester
roles by constructing `ScriptedModelClient` instances and passing them through
`ThreadRuntime`. That makes a deterministic distribution workflow look like a
mocked model turn path.

`ScriptedModelClient` is appropriate for unit and integration tests, but it
should not be the current production execution mechanism for the software
distribution. The forward-only contract should make deterministic workflow
steps explicit as kernel/tool operations rather than hiding them behind a fake
model client.

## Future-Roadmap Gap

This pass does not replace the deterministic CLI `code` workflow with a live
LLM supervisor. It only removes the scripted model client from the production
software distribution execution path. Live LLM goal-driven coverage remains in
the live e2e tests.

## Intended Fix Scope

- Replace production `ScriptedModelClient` use in software role execution with
  explicit kernel/tool workflow operations:
  role start, environment attach, capability grant, tool proposal, tool invoke,
  artifact commit, role final submission, thread completion, and checkpoint.
- Keep `ScriptedModelClient` available for tests and deterministic runtime
  coverage.
- Preserve software distribution reports and conformance behavior.

## Validation Planned

- `rg` check that `ScriptedModelClient` no longer appears in
  `crates/agent-os-thread/src/software`.
- Focused software distribution conformance tests.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
