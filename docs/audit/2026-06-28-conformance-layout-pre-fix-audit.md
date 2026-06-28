# Conformance Layout Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress roadmap-gap fix
  set, the agent-control split changes, and existing audit records.

## Scope

This continuation addresses the remaining testing-strategy gap documented in
`docs/audit/2026-06-28-testing-strategy-post-fix-audit.md`: flat conformance
test targets were retained for compatibility and deferred gradual migration.

The maintainer now directs the project to use a greenfield, unreleased,
forward-only contract. Therefore this fix must remove the compatibility-oriented
test layout and use one canonical conformance integration-test entrypoint.

## Current-Contract Gap

- `AGENTS.md` says integration tests belong under
  `crates/agent-os-conformance/tests/integration/` with a small top-level test
  target that imports directory modules.
- Current state still has flat root-level conformance targets:
  `artifact_conformance.rs`, `communication_conformance.rs`,
  `context_conformance.rs`, `lifecycle_conformance.rs`,
  `openai_adapter_conformance.rs`, `resource_provider_storage_conformance.rs`,
  `runtime_resume_conformance.rs`, `security_conformance.rs`,
  `software_distribution_conformance.rs`, and
  `state_replay_export_conformance.rs`.
- The post-fix testing audit explicitly describes that layout as retained for
  compatibility. That conflicts with the current forward-only objective.

## Intended Fix Scope

- Move the flat conformance scenarios into
  `crates/agent-os-conformance/tests/integration/`.
- Keep `crates/agent-os-conformance/tests/integration_tests.rs` as the single
  canonical integration test target.
- Update module declarations and `common` imports directly, without adding
  wrapper modules or compatibility targets.
- Update the testing audit record to state that the compatibility layout gap is
  resolved.

## Validation Planned

- `cargo fmt`
- `cargo test -p agent-os-conformance --test integration_tests`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
