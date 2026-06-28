# Agent Control Rejection Error Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers `agent_control` error semantics for actions rejected by the
current append-only store contract and for impossible lifecycle dispatch arms.

## Current-Contract Gaps

- `delete_session` and `purge_state` are current model-visible actions whose
  append-only-store behavior is a deterministic rejection path. The kernel and
  tests currently classify that rejection as `AgentOsError::Unsupported`, which
  implies an absent implementation instead of an intentional current-contract
  rejection.
- `apply_lifecycle_action` returns `AgentOsError::Unsupported` for actions that
  should never be dispatched to the lifecycle module. This is an invalid
  dispatch guard, not an unsupported lifecycle feature.

## Future-Roadmap Gap

No `agent_control` rejection-error item is intentionally deferred in this scope.

## Intended Fix Scope

- Reclassify append-only-store `delete_session` and `purge_state` rejection as
  `AgentOsError::Validation`.
- Reclassify impossible lifecycle dispatch as `AgentOsError::Validation`.
- Update conformance and live e2e test assertions to expect the explicit
  append-only-store rejection.

## Validation Planned

- Focused conformance test for privileged `agent_control` rejections.
- Focused compile check for `agent-os-thread` live tests.
- `cargo fmt --check`
- `git diff --check`
