# Agent-OS v0.2.0 Goal Control Post-Fix Audit

Date: 2026-06-29

## Snapshot

- Starting Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Starting Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Pre-fix audit:
  `docs/audit/2026-06-29-v0-2-goal-control-pre-fix-audit.md`
- Worktree note: this change was applied on top of an already dirty tree that
  included provider/env configuration edits and related docs. Those unrelated
  changes were preserved.

## Implemented Fixes

- Advanced workspace package version and ABI version to `0.2.0`.
- Replaced model-visible `set_objective` with Supervisor-only `set_goal`.
- Added model-visible `accomplish_goal`.
- Renamed thread task binding from `local_goal` to `goal` and added explicit
  goal status, revision, and accomplishment timestamp fields.
- Renamed invocation edge child-visible work field from `assignment` to `goal`.
- Changed `agent_control(action=start)` to require `payload.goal`; no
  compatibility alias is retained.
- Preserved the creation-time child goal across the child ACB and invocation
  edge.
- Implemented Supervisor retargeting with `set_goal` for the Supervisor's own
  thread or a direct child only; WorkerAgent attempts are denied by the kernel.
- Implemented `accomplish_goal` as a local goal completion action that marks the
  caller goal accomplished, completes active hooks, completes the invocation,
  and moves the thread to `Completing` so `submit_final` remains the final tool
  call.
- Updated successful `submit_final` handling to close active hooks and active
  invocation state for the current thread.
- Converted `delete_session` into an applied replayable lifecycle action that
  records a command, clears active runtime/session state, unloads the thread,
  and allows `resume` to create a new session id.
- Converted `purge_state` into an applied replayable lifecycle action that
  records a command, terminates the target projection, cancels active hooks,
  closes invocation state, and emits an `AgentStatePurged` tombstone event.
- Updated OpenAI-compatible and Anthropic-compatible schemas, parser mapping,
  prompt text, message reconstruction, mock adapter coverage, ignored live e2e
  prompts, README, and current design/implementation docs.

## Validation Results

- `cargo check --workspace --message-format short`
  - Passed.
- `cargo test --workspace --message-format short --no-run -j 1`
  - Passed.
- `cargo test --workspace --message-format short -j 1`
  - Passed: 13 CLI tests, 68 conformance integration tests, 5 kernel tests, 3
    kernel-unit tests, 2 store tests, 3 SQLite store tests, 1 sys test, 28
    thread tests; 10 live LLM tests ignored by credential gates; doctests
    passed.
- `cargo fmt --all`
  - Passed.
- `cargo clippy --workspace --all-targets -- -D warnings`
  - Passed.
- `git diff --check`
  - Passed, with only Git's Windows LF-to-CRLF working-copy warnings.

During validation, the first full test run exposed a deadlock in `set_goal`
caused by holding a projection read lock while emitting an invocation update.
The implementation now clones the invocation before emitting, releasing the
read lock before the write-side event application.

## Changed File Areas

- ABI/types: `crates/agent-os-sys/src/core.rs`,
  `crates/agent-os-sys/src/lifecycle.rs`
- Kernel lifecycle and tool broker:
  `crates/agent-os-kernel/src/threads.rs`,
  `crates/agent-os-kernel/src/events.rs`,
  `crates/agent-os-kernel/src/artifacts.rs`,
  `crates/agent-os-kernel/src/inputs.rs`,
  `crates/agent-os-kernel/src/profile_seed/tools.rs`,
  `crates/agent-os-kernel/src/tools/driver.rs`,
  `crates/agent-os-kernel/src/tools/driver/work_state.rs`,
  `crates/agent-os-kernel/src/tools/driver/agent_control/mod.rs`,
  `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`
- Runtime/model adapters:
  `crates/agent-os-thread/src/openai/tools.rs`,
  `crates/agent-os-thread/src/openai/parser.rs`,
  `crates/agent-os-thread/src/openai/messages.rs`,
  `crates/agent-os-thread/src/openai/prompt.rs`,
  OpenAI test fixtures and live ignored scenarios.
- Conformance tests:
  lifecycle, tool broker, artifact, communication, runtime goal-driven, and
  related spawn input updates.
- Docs:
  `README.md`, `docs/README.md`, current `docs/10-kernel-design/**`, and
  `docs/20-implementation/**`.

## Forward-Only Notes

- No `set_objective` alias, `payload.assignment` alias, migration shim, or
  compatibility fallback was added.
- The physical event history remains append-only. Delete and purge are logical
  projection/lifecycle effects recorded by replayable events.
- `accomplish_goal` intentionally leaves the thread in `Completing`; this
  encodes the user rule that execution agents accomplish the local goal first
  and then call `submit_final`, with `submit_final` always last.

## Remaining Gaps

- Live OpenAI-compatible and Anthropic-compatible e2e tests were updated but not
  executed because provider credentials, network access, and spend approval are
  required.
- Broader distributed/production control-plane behavior remains out of scope
  for this v0.2.0 core closure.
