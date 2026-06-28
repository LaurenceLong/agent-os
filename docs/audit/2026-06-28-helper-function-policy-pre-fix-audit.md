# Helper Function Policy Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress roadmap-gap fix
  set, conformance-layout migration, provider-doc sync, agent-control split,
  forward-only rule update, and audit records.

## Scope

This audit covers project rules and current in-progress wording related to
helper functions.

The maintainer clarified that helper functions should be removed rather than
kept as an accepted implementation pattern.

## Current-Contract Gap

`AGENTS.md` still says helper functions may be introduced when they reduce
complexity. That wording conflicts with the clarified implementation contract:
new extracted functions must represent domain operations or ownership
boundaries, not generic helpers.

The prior AGENTS forward-only audit also records the superseded helper-function
allowance. Two test modules also use `compile_helper` fixture functions for
external model binaries, and one in-progress `agent_control` error string
describes rejected actions as "helper" actions.

## Future-Roadmap Gap

No helper-function policy items are intentionally deferred in this scope.

## Intended Fix Scope

- Update `AGENTS.md` so helper functions are not an allowed implementation
  pattern.
- Remove helper-function allowance wording from the AGENTS forward-only audit.
- Inline the external model binary compilation fixture steps in the tests that
  currently use `compile_helper`.
- Remove the "helper action" wording from the in-progress `agent_control`
  lifecycle module.
- Keep this pass focused on helper-function policy and current in-progress
  wording.

## Validation Planned

- `rg` checks for helper-function policy wording.
- Focused cargo checks for touched Rust code when needed.
- `cargo fmt --check`
- `git diff --check`
