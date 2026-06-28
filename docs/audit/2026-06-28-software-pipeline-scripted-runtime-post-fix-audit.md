# Software Pipeline Scripted Runtime Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status after fix: dirty with the forward-only roadmap
  implementation, conformance layout moves, and audit records.

## Implemented Fixes

- Replaced production software role execution through deterministic scripted
  model turns with explicit kernel/tool workflow operations.
- Explorer, Coder, and Tester now start role turns, attach environments, grant
  tool capability, record tool proposals, invoke tools, submit role finals, mark
  threads completed, and record checkpoints directly.
- Coder patch artifact creation now fails closed when the edit tool omits
  completion, evidence, output, or the expected changed path.
- Tool-role final submission now requires evidence claims for evidenced tool
  results instead of inventing a default claim.
- Removed `ScriptedModelClient` and `ScriptedStep` from the public
  `agent-os-thread` crate surface; deterministic model behavior now lives in
  tests only.

## Changed Files

- `crates/agent-os-thread/src/software/roles.rs`
- `crates/agent-os-thread/src/software/tool_workflow.rs`
- `crates/agent-os-thread/src/software/mod.rs`
- `crates/agent-os-thread/src/lib.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-conformance/tests/common/mod.rs`
- `crates/agent-os-conformance/tests/integration/openai_adapter_conformance.rs`
- `crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs`
- `crates/agent-os-conformance/tests/integration/runtime_resume_conformance.rs`
- `crates/agent-os-thread/src/scripted.rs`

## Validation Results

- `rg -n "ScriptedModelClient|ScriptedStep|scripted" AGENTS.md crates docs\00-foundation docs\10-kernel-design docs\20-implementation docs\30-decisions --glob '!target/**' --glob '!docs/audit/**'`
  returned only the AGENTS e2e prohibition.
- `cargo test -p agent-os-thread runtime --lib` passed.
- `cargo test -p agent-os-conformance --test integration_tests openai_adapter`
  passed.
- `cargo test -p agent-os-conformance --test integration_tests runtime` passed.
- `cargo test -p agent-os-conformance --test integration_tests software_distribution_runs_through_required_roles`
  passed.
- `cargo test -p agent-os-thread software_pipeline_runs_all_roles_and_submits_supervisor_final --lib`
  passed.
- `cargo test --workspace` passed: CLI, conformance integration, kernel,
  store, sqlite, sys, thread, and doc-tests. Live LLM e2e tests remained
  ignored because they require provider credentials.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo fmt --check` passed.
- `git diff --check` passed with only line-ending warnings.

## Compatibility Notes

This is an intentional forward-only change for an unreleased project. No
backward compatibility alias, scripted production fallback, or public scripted
test adapter was retained.

## Remaining Gaps

No remaining current-contract gap for production software distribution scripted
runtime usage.
