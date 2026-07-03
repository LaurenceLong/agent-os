# PTY And Process Lifecycle Parity Plan

Status: planning

Last updated: 2026-07-03

## Goal

Make command execution a first-class Agent-OS process lifecycle subsystem rather
than a one-shot tool implementation. The target is Codex-like parity for PTY
sessions, stdin continuation, output windows, interruption, termination,
foreground/background transitions, and cross-platform behavior.

## Non-Goals

- Do not wrap older process behavior to preserve old semantics.
- Do not make the Agent Thread runtime own process authority.
- Do not introduce a PTY dependency without an ADR.
- Do not preserve historical persisted process state from older schemas.

## Current Agent-OS State

`run_command` is model-visible and kernel-mediated. It validates workspace cwd,
uses a compatible active environment lease, runs either a shell command or direct
exec, captures bounded stdout/stderr, stores spool paths, and emits tool
progress/completion events. Process execution now creates durable
`ProcessSession` records with process ids, command metadata, stdin mode,
output stream sequence state, retained output chunks, stdin write records,
orphan recovery, process list/read projection, stop, kill, and app/CLI process
list/stop/kill flows. `write_stdin` is the canonical model-visible continuation
tool for writing to or polling a process started by the invoking agent.

The current gap is lifecycle continuity:

- no PTY mode;
- no executor-backed process placement protocol;
- no direct CLI `write_stdin` UX;
- stop/kill is available through app/CLI and supervisor process control, but
  model-visible interruption still flows through `agent_control`;
- live LLM e2e coverage for process continuation remains incomplete.

## Codex Reference

Codex uses `exec_command` and `write_stdin` over a unified process manager. A
process can continue after the first yield, retain output, accept stdin, expose
exit state, and be terminated or interrupted. The same protocol shape can run
locally or through an executor process API.

## Target Agent-OS Contract

Agent-OS should define `ProcessSession` as the canonical runtime object:

- `process_id`
- owning thread/session ids
- workspace root and cwd
- argv or shell command
- environment policy and injected environment
- TTY and stdin mode
- sandbox and permission decision references
- lifecycle state: starting, running, exited, failed, interrupted, terminated,
  timed out, orphaned
- output streams with monotonically increasing sequence numbers
- bounded retained output plus managed spool references
- foreground yield policy and background continuation policy

Model-visible tools should converge on:

- `run_command`: starts a process, waits for an initial yield, and returns output
  plus `process_id` when the process remains alive.
- `write_stdin`: writes to or polls an existing process session.

Kernel/internal APIs should also support list, interrupt, terminate, and cleanup.
Those may start as runtime/app-server operations before becoming model-visible
tools.

## Crate Ownership

- `agent-os-sys`: shared process session/event/output data types.
- `agent-os-kernel`: process manager, lifecycle reducer, permission mediation,
  output retention, evidence attachment, replay behavior, worker recovery.
- `agent-os-thread`: model action parsing and runtime calls into kernel tools.
- `agent-os-store`: event and blob storage traits only if process events become
  durable.
- `agent-os-cli`: human-facing formatting for process continuation and cleanup.

## Implementation Slices

1. Add the process ABI and kernel reducer state.
   Implemented.
2. Replace the current `run_command` driver internals with the kernel process
   manager while keeping one canonical model-visible start tool.
   Implemented for local shell and exec processes.
3. Add output sequence storage, retained output windows, and poll semantics.
   Implemented for process output chunks and `write_stdin`/`agent_control`
   polling.
4. Add stdin writes with write ids for idempotent retry.
   Implemented through `ProcessStdinWrite` and the model-visible `write_stdin`
   tool.
5. Add interrupt and terminate semantics.
   Implemented through process stop/kill app and supervisor control paths.
6. Add PTY backend behind an explicit ADR.
7. Add app-server/CLI process listing and cleanup.
   Process list, stop, and kill are implemented; direct stdin CLI UX remains.
8. Add model-visible continuation tests and live runtime coverage.
   Conformance covers `run_command` followed by `write_stdin`; live LLM e2e
   coverage remains.

## Validation

- Unit tests for process state transitions, output sequence ordering, stdin
  idempotency, pruning, timeout, interrupt, terminate, Windows shell mode, and
  Unix shell mode.
- Kernel/store integration tests for event replay, orphan recovery, and managed
  output reads.
- Runtime conformance tests for `run_command` followed by `write_stdin`.
- Ignored live LLM e2e tests where the model starts a long-running command,
  polls it, sends input, and finalizes from observed output.

## Forward-Only Notes

The current background worker shape should be removed or narrowed once
`ProcessSession` exists. It should not remain as a parallel long-running command
path.

`ProcessSession` is now the canonical process state. Remaining work should
extend that subsystem rather than adding another command/session abstraction.
