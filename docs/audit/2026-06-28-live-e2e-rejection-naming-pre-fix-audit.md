# Live E2E Rejection Naming Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only build
  changes and audit records.

## Scope

This audit covers live LLM e2e naming for `agent_control` actions that the
append-only store must reject.

## Current-Contract Gap

- `AGENTS.md` now describes `delete_session` and `purge_state` coverage as
  append-only-store rejection paths.
- `crates/agent-os-thread/src/openai/tests/live.rs` still names the live tests,
  test support functions, temporary directories, audit log files, JSON log
  events, titles, and descriptions as "unsupported" action coverage.
- The same live fixture uses `local_goal: "placeholder"` for supervisor agents
  before constructing the model-visible task snapshot.
- `docs/audit/2026-06-28-live-llm-e2e-policy-post-fix-audit.md` still describes
  the same live negative scenario as unsupported append-only-store actions.

The behavior is correct fail-closed rejection behavior, but the terminology
suggests a temporary missing implementation.

## Intended Fix Scope

- Rename the live e2e tests and test support functions from `unsupported` to
  `rejection`.
- Rename related audit log files, JSON log event types, printed labels, titles,
  descriptions, and target labels.
- Update the live LLM e2e policy audit wording to "rejection".
- Replace fixture placeholder goals with concrete live scenario goals.
- Keep the current `AgentOsError::Unsupported` match because the error enum name
  is the current low-level error variant.
- Do not add aliases for the old test names.

## Validation Planned

- `cargo test -p agent-os-thread --lib --no-run`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
