# Scripted Model Public Surface Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status after fix: dirty with the forward-only roadmap
  implementation and audit records.

## Implemented Fixes

- Removed the production `scripted` module from `agent-os-thread`.
- Removed public exports for `ScriptedModelClient` and `ScriptedStep`.
- Added deterministic model fixtures inside runtime unit tests and conformance
  test support.
- Deterministic test final submission now rejects evidenced tool results that
  lack evidence claims instead of fabricating default claims.

## Changed Files

- `crates/agent-os-thread/src/lib.rs`
- `crates/agent-os-thread/src/scripted.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-conformance/tests/common/mod.rs`
- `crates/agent-os-conformance/tests/integration/openai_adapter_conformance.rs`
- `crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs`
- `crates/agent-os-conformance/tests/integration/runtime_resume_conformance.rs`

## Validation Results

- `rg -n "ScriptedModelClient|ScriptedStep|scripted" AGENTS.md crates docs\00-foundation docs\10-kernel-design docs\20-implementation docs\30-decisions --glob '!target/**' --glob '!docs/audit/**'`
  returned only the AGENTS e2e prohibition.
- `cargo test -p agent-os-thread runtime --lib` passed.
- `cargo test -p agent-os-conformance --test integration_tests openai_adapter`
  passed.
- `cargo test -p agent-os-conformance --test integration_tests runtime` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo fmt --check` passed.
- `git diff --check` passed with only line-ending warnings.

## Compatibility Notes

This is an intentional public surface cleanup for an unreleased project. No
backward compatibility export or replacement alias was kept.

## Remaining Gaps

No remaining current-contract gap for scripted model public API exposure.
