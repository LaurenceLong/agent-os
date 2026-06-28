# Conformance Layout Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-conformance-layout-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Removed the flat root-level conformance test target layout.
- Moved the conformance scenario files into
  `crates/agent-os-conformance/tests/integration/`.
- Kept `crates/agent-os-conformance/tests/integration_tests.rs` as the single
  canonical conformance integration test target.
- Updated `crates/agent-os-conformance/tests/integration/mod.rs` to import each
  scenario module directly.
- Updated moved tests to use the crate-root `common` module instead of declaring
  their own per-target `mod common`.
- Updated `docs/audit/2026-06-28-testing-strategy-post-fix-audit.md` so the
  old compatibility-layout gap no longer reads as open.

## Changed Files

- `crates/agent-os-conformance/tests/integration/mod.rs`
- `crates/agent-os-conformance/tests/integration/artifact_conformance.rs`
- `crates/agent-os-conformance/tests/integration/communication_conformance.rs`
- `crates/agent-os-conformance/tests/integration/context_conformance.rs`
- `crates/agent-os-conformance/tests/integration/lifecycle_conformance.rs`
- `crates/agent-os-conformance/tests/integration/openai_adapter_conformance.rs`
- `crates/agent-os-conformance/tests/integration/resource_provider_storage_conformance.rs`
- `crates/agent-os-conformance/tests/integration/runtime_resume_conformance.rs`
- `crates/agent-os-conformance/tests/integration/security_conformance.rs`
- `crates/agent-os-conformance/tests/integration/software_distribution_conformance.rs`
- `crates/agent-os-conformance/tests/integration/state_replay_export_conformance.rs`
- `docs/audit/2026-06-28-testing-strategy-post-fix-audit.md`
- `docs/audit/2026-06-28-conformance-layout-pre-fix-audit.md`
- `docs/audit/2026-06-28-conformance-layout-post-fix-audit.md`

Removed root-level compatibility targets:

- `crates/agent-os-conformance/tests/artifact_conformance.rs`
- `crates/agent-os-conformance/tests/communication_conformance.rs`
- `crates/agent-os-conformance/tests/context_conformance.rs`
- `crates/agent-os-conformance/tests/lifecycle_conformance.rs`
- `crates/agent-os-conformance/tests/openai_adapter_conformance.rs`
- `crates/agent-os-conformance/tests/resource_provider_storage_conformance.rs`
- `crates/agent-os-conformance/tests/runtime_resume_conformance.rs`
- `crates/agent-os-conformance/tests/security_conformance.rs`
- `crates/agent-os-conformance/tests/software_distribution_conformance.rs`
- `crates/agent-os-conformance/tests/state_replay_export_conformance.rs`

## Forward-Only Notes

This is an intentional forward-only layout change. No alternate test targets or
gradual migration aliases remain for these conformance scenarios.

## Validation Results

- `cargo fmt`: passed.
- `cargo test -p agent-os-conformance --test integration_tests`: passed, 66
  tests.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract conformance-layout gaps remain for this audit scope.
