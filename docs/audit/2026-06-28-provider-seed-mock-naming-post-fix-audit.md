# Provider Seed Mock Naming Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-provider-seed-mock-naming-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Renamed production provider seed IDs from mock-oriented names to neutral
  current-contract names:
  - `mock-provider` -> `primary-provider`
  - `mock-model` -> `general-primary`
  - `mock-coding-primary` -> `primary-coding-model`
  - `mock-review-primary` -> `primary-review-model`
  - `mock-text-only` -> `primary-text-model`
- Updated thread config snapshots from `mock-provider`, `mock-model`, and
  `mock-0.1` to `primary-provider`, `general-primary`, and
  `provider-adapter-0.1`.
- Updated Provider System documentation to match the current seed catalog.
- Updated conformance assertions that inspect provider IDs.
- Left mock adapter test fixtures unchanged because they intentionally exercise
  mocked provider responses.

## Changed Files

- `crates/agent-os-kernel/src/profile_seed/provider.rs`
- `crates/agent-os-kernel/src/threads.rs`
- `crates/agent-os-conformance/tests/integration/resource_provider_storage_conformance.rs`
- `docs/10-kernel-design/provider-system.md`
- `docs/audit/2026-06-28-provider-seed-mock-naming-pre-fix-audit.md`
- `docs/audit/2026-06-28-provider-seed-mock-naming-post-fix-audit.md`

## Forward-Only Notes

The production provider catalog now reads as a current Agent-OS default
contract rather than as a test fixture. Mock naming remains confined to mock
adapter tests.

## Validation Results

- `rg -n "mock-provider|mock-model|mock-coding-primary|mock-review-primary|mock-text-only|mock-0\\.1" crates docs distros --glob '!docs/audit/**' --glob '!target/**'`:
  matches remain only in `crates/agent-os-thread/src/openai/tests/mock_adapter.rs`.
- `cargo test -p agent-os-conformance --test integration_tests provider_routing_uses_role_policy_and_model_aliases`:
  passed.
- `cargo test -p agent-os-conformance --test integration_tests scheduler_rejects_turn_when_provider_slot_is_unavailable`:
  passed.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract provider seed mock-naming gaps remain for this audit scope.
