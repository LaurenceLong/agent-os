# AGENTS Forward-Only Rules Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress roadmap-gap fix
  set, conformance-layout migration, provider-doc sync, agent-control split
  changes, and audit records.

## Scope

This audit covers the project-level contributor rules in `AGENTS.md`.

The maintainer directs this build to treat the repository as a greenfield,
unreleased system and to avoid backward compatibility, fallbacks, compatibility
layers, legacy adapters, deprecated paths, migration shims, feature flags, and
temporary workarounds.

## Current-Contract Gap

`AGENTS.md` still defines the opposite contract:

- It says the repository is already released and is not a greenfield rewrite.
- It has a "Non-Negotiable Compatibility Rules" section.
- It requires preserving supported behavior, fallbacks, compatibility aliases,
  legacy code paths, old public APIs, old CLI flags, old environment variables,
  serialized JSON shapes, provider adapter styles, and profile identifiers.
- It requires keeping compatibility shims until a replacement path is safe for
  existing state.
- Its implementation-style section tells agents to keep compatibility aliases
  and fallbacks.
- Its storage section requires tolerance for older persisted records.
- Its audit workflow asks for compatibility notes after fixes.

These rules conflict with the current forward-only objective and would keep
future work biased toward the old released-project posture.

## Intended Fix Scope

- Reframe `AGENTS.md` as a greenfield, unreleased, forward-only project.
- Replace compatibility rules with one canonical forward design rule set.
- Keep architecture, testing, audit, and dirty-tree rules that still apply.
- Update implementation style with the maintainer's direct-code guidance:
  extracted functions should represent domain operations or meaningful
  boundaries, not generic helper functions.
- Update storage rules so current schemas and current replay remain
  deterministic without requiring legacy persisted-state compatibility.
- Update audit wording from compatibility notes to forward-only notes.

## Validation Planned

- `rg` checks for removed compatibility-rule wording in `AGENTS.md`.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
