# Long-Running Kernel and App Server Gap Analysis

Status: transitional design record
Created: 2026-06-30

## 1. Purpose

This document records the target architecture correction for Agent-OS before the
code is fully converged. It is intentionally additive: it does not rewrite or
supersede existing normative documents under `docs/10-kernel-design/`.

The existing architecture docs should remain stable until the implementation
has a real long-running kernel boundary, app-server boundary, and durable
projection model. After that code lands and passes conformance, the older
architecture docs can be updated in one coherent pass.

## 2. External Product Reference

OpenAI Codex is a useful product reference because it separates a rich client
experience from the agent runtime machinery:

- Codex app features: <https://developers.openai.com/codex/app/features>
- Codex app automations: <https://developers.openai.com/codex/app/automations>
- Codex app-server: <https://developers.openai.com/codex/app-server>

Relevant reverse-engineered requirements from that surface:

- desktop and terminal clients need to work with many threads across projects;
- worktrees are a first-class isolation mode for parallel work;
- a thread has turns, items, streamed updates, diffs, plans, token usage, and
  artifacts;
- approvals, sandbox settings, MCP, skills, browser use, computer use,
  integrated terminals, Git operations, and automations need one common control
  plane;
- rich clients should connect through an app-server style API with typed
  request/response messages and streamed notifications.

## 3. Corrected Architecture Thesis

Agent-OS is a long-running kernel control plane. Agent Threads are task
execution units managed by that kernel. Terminal UI, desktop app, IDE, cloud UI,
and automations are clients outside the kernel.

The kernel must own durable state and state transitions. Agent Thread runtimes
may execute model loops, provider calls, and tool loops, but they must request
kernel operations instead of owning task state directly.

```text
Terminal UI / Desktop App / IDE / Cloud UI / Automations
        |
        v
Agent-OS App Server
  - typed client protocol
  - client identity and capabilities
  - request/response methods
  - streamed notifications
        |
        v
Agent-OS Kernel Daemon
  - thread/task/turn lifecycle
  - scheduler and resource arbitration
  - permissions and approval routing
  - provider policy, budget, usage, and rate-limit state
  - tool broker and execution-environment leases
  - artifact/evidence index
  - durable projections and statistics
  - automation scheduler and wakeups
        |
        v
Agent Thread Runtime Workers
  - model/client adapters
  - prompt and tool-call loop
  - local/remote tool execution drivers through kernel grants
  - cooperative yield boundaries
        |
        v
Store Drivers
  - append-only events
  - typed projection tables
  - idempotency records
  - blob stores for evidence and artifacts
```

## 4. Naming Model

The implementation should keep two concepts separate:

- Client-facing thread/session: the conversation or task container a terminal
  UI or desktop app opens, resumes, searches, archives, or forks.
- Agent Thread: the governed execution unit with an Agent Control Block, role,
  permission profile, scheduler binding, environment leases, and task binding.

A client-facing thread contains turns and items. Internally, turns map to
kernel-managed task execution intervals. Agent Threads may be assigned to run a
turn, delegate work, or review another Agent Thread's output.

This separation prevents desktop-app terminology from collapsing the kernel
runtime model into a chat loop.

## 5. Required Client-Facing Primitives

The app-server should expose stable primitives for rich clients. The exact wire
format can be JSON-RPC style, but the contract must be typed in `agent-os-sys`
or a narrow protocol crate.

Minimum lifecycle methods:

- initialize client connection with client identity and declared capabilities;
- start, resume, fork, read, list, search, archive, unarchive, rename, and
  delete client-facing threads;
- start, steer, interrupt, and observe turns;
- subscribe and unsubscribe to thread, turn, item, approval, terminal, and
  statistics notifications;
- read paginated turns and timeline items;
- submit approval decisions and user input;
- query current projections and operational statistics.

Minimum streamed item types:

- user message;
- agent message delta and completed message;
- plan update;
- tool call proposed, started, completed, failed, denied;
- command output chunk and command completion;
- file change and turn-level diff update;
- approval request and approval result;
- artifact committed;
- evidence attached;
- provider usage update;
- token usage update;
- budget warning and budget exhausted;
- context compaction;
- automation run started, completed, failed, archived.

## 6. Durable Projection Model

Append-only events remain the replay source of truth. They are not sufficient as
the user-facing query surface.

The store needs typed projection tables that are updated by kernel reducers in
the same logical write path as event appends. Projections must be rebuildable
from current event streams, deterministic for the current schema, and queryable
without re-parsing arbitrary event JSON on every UI request.

Required projection families:

- thread/session summary projection;
- turn summary projection;
- item timeline projection;
- agent control block projection;
- task and goal projection;
- provider usage projection;
- token, cost, cache, and rate-limit projection;
- tool invocation outcome and latency projection;
- budget ledger projection;
- approval queue projection;
- environment, terminal, browser, and worktree lease projection;
- artifact and evidence index projection;
- automation schedule and run projection;
- projection checkpoint and rebuild status.

Statistics belong here. CLI, desktop app, and automations should query these
read models instead of each rebuilding their own metrics from raw events or
audit JSONL.

## 7. Gap Register

| Gap | Current implementation shape | Target shape | Implementation direction |
| --- | --- | --- | --- |
| G-01 Kernel process model | `Kernel` is an in-process Rust struct opened by CLI commands. | A long-running `agent-os-kerneld` owns state, scheduling, resources, and subscriptions. | Add a daemon/service boundary that loads the store once, owns the runtime registry, and exposes a local control endpoint. |
| G-02 App-server boundary | CLI calls kernel and `ThreadRuntime` directly. | Terminal UI, desktop app, IDE, and automation clients all use one app-server protocol. | Add an app-server crate with typed initialize, thread, turn, item, approval, stats, and subscription APIs. |
| G-03 Thread terminology | CLI task processing can read like one prompt equals one Agent Thread run. | Client-facing threads contain turns/items; Agent Threads are kernel-managed execution units. | Add explicit session/thread, turn, and item read models while preserving Agent Thread as the internal governed executor. |
| G-04 Runtime ownership | CLI constructs model client and runs `ThreadRuntime::run_to_completion`. | Kernel schedules Agent Thread runtime workers and controls turn lifecycle. | Move runtime start/resume/interrupt behind kernel-managed jobs and worker leases. |
| G-05 Projection storage | SQLite has events and idempotency records; `ProjectionStore` reads event JSON by aggregate type. | SQLite has typed projection tables plus projection checkpoints. | Add schema version with projection tables and reducers; keep replay deterministic for the current event model. |
| G-06 Statistics | Usage and tool events exist, but no unified stats query surface exists. | Stats are durable kernel projections by thread, turn, agent, provider, model, tool, workspace, and benchmark. | Add stats reducers for token/cost/cache/tool/latency/budget dimensions and expose app-server queries. |
| G-07 Live event stream | Status reads snapshots and event counts; no shared subscription model exists. | Clients receive typed notifications for thread, turn, item, approval, stats, and resource changes. | Add event fanout from kernel write path to app-server subscriptions with replay cursor support. |
| G-08 Automation scheduler | No first-class unattended scheduled run model is present. | Kernel owns thread wakeups, standalone recurring runs, project runs, and automation inbox/triage state. | Add automation schedules, run records, scheduler wakeups, worktree selection, and archived/no-finding outcomes. |
| G-09 Resource sessions | Execution environments exist, but terminal/browser/worktree/client sessions are not all first-class read models. | Terminal, browser, worktree, SSH/device, and local app control are kernel resources with leases and permissions. | Add typed resource descriptors and lease projections for each resource family. |
| G-10 Approval routing | Permission grants and requests exist, but connected client identity and app approval routing are not complete. | Human S0 clients, S1 root agents, and deeper Agent Threads use one approval routing and capability model. | Bind app-server clients to identities, route approval requests through kernel queues, and persist scoped approval decisions. |
| G-11 Artifact and evidence UI | Artifact/evidence metadata exists but rich-client timeline and preview projections are incomplete. | Clients can inspect artifacts, evidence, generated files, logs, screenshots, and summaries through typed indexes. | Add item timeline entries and artifact/evidence index queries designed for UI previews. |
| G-12 Provider operations | Provider config and usage events exist, but operational status is not a durable UI surface. | Kernel exposes provider profile, model alias, usage, cost, cache, rate-limit, retry, and error projections. | Expand provider usage schema and projection reducers; expose provider status and stats queries. |
| G-13 Git and review surface | Git-related behavior is tool-driven and CLI-oriented. | App clients can show diffs, comments, staged changes, commits, pushes, PR creation, and review outcomes. | Add Git/review projection items and resource-scoped operations through tool broker or dedicated kernel actions. |
| G-14 Conformance coverage | Existing conformance focuses on kernel/runtime/tool contracts, not app-server and projections. | Conformance covers daemon restart, projection rebuild, app-server lifecycle, streaming, approval routing, and multi-client behavior. | Add focused integration tests under `crates/agent-os-conformance/tests/integration/`. |
| G-15 Old documentation accuracy | Normative docs already describe much of the intended architecture, but implementation is not fully caught up. | Normative docs describe the architecture only after the code proves it. | Keep this document as the interim record; update old docs after implementation and conformance pass. |

## 8. Implementation Order

The implementation should converge in this order:

1. Projection foundation: add typed projection tables, reducers, and rebuild
   tests for current event streams.
2. Statistics projection: add token, cost, cache, provider, tool, latency,
   budget, and benchmark-oriented read models.
3. Kernel daemon boundary: add a long-running kernel process with startup,
   shutdown, store locking, replay, projection rebuild, and event fanout.
4. App-server protocol: add initialize, thread, turn, item, approval, stats, and
   subscription APIs.
5. Runtime worker boundary: make Agent Thread runtime execution a
   kernel-scheduled worker job instead of direct CLI orchestration.
6. Resource session expansion: add terminal, browser, worktree, Git/review, and
   artifact/evidence UI read models.
7. Automation scheduler: add thread wakeups, project automations, standalone
   runs, triage state, and archive/no-finding outcomes.
8. CLI migration: convert CLI chat/status paths to use the same app-server or
   kernel-service contract as richer clients.

This order keeps the data model ahead of the UI surface. A desktop app or
terminal UI should not need private event parsing once it starts consuming the
new architecture.

## 9. Old-Document Update Gate

Do not update the existing normative architecture docs to describe this target
as current implementation until these gates pass:

- SQLite has durable typed projection tables beyond events and idempotency.
- Projection reducers can rebuild thread, turn, item, stats, approval, and
  resource read models from the current event stream.
- A long-running kernel service owns the store and event fanout.
- App-server APIs can start/resume/read/list/search client-facing threads and
  start/steer/interrupt turns.
- Agent Thread runtime execution is scheduled through the kernel boundary.
- CLI status and chat no longer need to duplicate rich-client state assembly.
- Conformance tests cover daemon restart, projection rebuild, app-server
  lifecycle, streaming notifications, approval routing, statistics queries, and
  at least one multi-client subscription scenario.

After these gates pass, update the older documents in one pass:

- `docs/10-kernel-design/system-architecture.md`
- `docs/10-kernel-design/kernel-abi-and-syscalls.md`
- `docs/10-kernel-design/state-storage-and-replay.md`
- `docs/10-kernel-design/agent-thread-runtime.md`
- `docs/10-kernel-design/execution-environment-system.md`
- `docs/20-implementation/production-roadmap.md`
- `docs/05-research/agent-optimization-statistics-study.md`

## 10. Forward-Only Notes

This repo is unreleased. The implementation should choose the clean current
contract over compatibility with older local databases, older CLI behavior, or
older event payloads.

Schema changes should still be deterministic and tested for the current schema.
Historical compatibility with older persisted state is not required for this
build.

The app-server contract should be canonical once introduced. Terminal UI,
desktop app, IDE integrations, and automations should not grow separate private
control paths.
