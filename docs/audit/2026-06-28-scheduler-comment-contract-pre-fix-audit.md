# Scheduler Comment Contract Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation and audit records.

## Scope

This audit covers the current-contract comment on cooperative ready-queue
ordering in `crates/agent-os-kernel/src/scheduler.rs`.

## Current-Contract Gap

`enqueue_ready` already sorts queued threads by task priority with a stable
thread-id tie break, and conformance covers this behavior in
`ready_queue_orders_ready_threads_by_task_priority`.

The doc comment still says the queue is ordered by insertion and that ordering
is a future scheduler concern. That comment contradicts the implemented
current contract.

## Future-Roadmap Gap

No scheduler comment item is intentionally deferred in this scope.

## Intended Fix Scope

- Update the `enqueue_ready` doc comment to describe current priority ordering.
- Do not change scheduler behavior.

## Validation Planned

- `rg` check for the stale scheduler-future wording.
- Focused conformance test for ready-queue priority ordering.
- `cargo fmt --check`
- `git diff --check`
