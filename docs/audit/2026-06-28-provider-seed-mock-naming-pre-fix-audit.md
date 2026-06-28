# Provider Seed Mock Naming Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers production provider seed identifiers and thread config
snapshot provider names.

## Current-Contract Gap

Production kernel seed data and thread config snapshots still use mock-oriented
provider names:

- `mock-provider`
- `mock-model`
- `mock-coding-primary`
- `mock-review-primary`
- `mock-text-only`
- `mock-0.1`

Mock naming is correct in adapter tests, but it should not be the production
default provider/catalog contract for a greenfield current design. Keeping it
there makes the default runtime profile read like a test fixture.

## Future-Roadmap Gap

No provider seed naming item is intentionally deferred in this scope.

## Intended Fix Scope

- Rename production seed provider/catalog identifiers to neutral current
  contract names.
- Update provider-system docs to match the seed data.
- Update conformance expectations that assert provider IDs.
- Leave test-only mock adapter fixtures unchanged.

## Validation Planned

- `rg` check for production `mock-provider` / `mock-model` / `mock-0.1`
  occurrences outside test-only mock adapter paths.
- Focused provider routing and provider-slot conformance tests.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
