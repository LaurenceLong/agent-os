# Agent-OS

Agent-OS is a production-oriented runtime kernel for agent organizations.

It is not a chatbot framework, not a workflow engine, and not a thin wrapper around existing multi-agent projects. The system goal is to make agents observable, schedulable, recoverable, auditable, governable, and composable in the same way an operating system makes processes and threads manageable.

The central execution unit is the Agent Thread. An Agent Thread is not an arbitrary open-source agent loop. It is a first-class runtime entity controlled by the Agent-OS kernel through an Agent Control Block, scoped context, capability-checked syscalls, evidence requirements, artifacts, audit events, and cooperative scheduling boundaries.

## Design Baseline

Current baseline: `v0.1-design + Rust single-node runtime skeleton`

This repository contains the design contract and an initial Rust implementation that follows it. Implementation work should not start by adopting LangGraph, AutoGen, CrewAI, OpenHands, or any other agent framework as the core runtime. Those projects can be studied, wrapped, or hosted as compatibility guests, but the Agent-OS kernel and Agent Thread Runtime must be owned by this project.

## Rust Workspace

The initial implementation is contract-first and single-node:

```text
crates/
  agent-os-sys/          # ABI types, data model, syscalls, events
  agent-os-store/        # append-only store traits and in-memory driver
  agent-os-store-sqlite/ # SQLite event/idempotency store driver
  agent-os-kernel/       # single-node microkernel services
  agent-os-thread/       # Agent Thread Runtime loop, model adapters, and workflow examples
  agent-os-conformance/  # conformance tests for the normative docs
  agent-os-cli/          # lifecycle demo and deterministic e2e task runner
```

Implemented kernel surfaces include:

- versioned syscall and event envelopes
- Phase 0 ABI/data schemas, including blackboard, context, memory, locks, package manifests, profiles, leases, artifacts, evidence, review, verification, and final submissions
- ATCB lifecycle and transition validation
- core role, permission, sandbox, scheduler, provider, and communication profiles
- capability intersection with permission profile ceilings
- append-only events, state replay, and trait-backed storage
- SQLite local store driver with schema migration tracking and idempotent syscall result persistence
- SQLite-backed CLI state with replayed projections and restart-safe ID allocation
- local hash-addressed artifact and evidence blob storage
- selected task and replay bundle export with profile snapshots, projection slices, and replayable event subsets
- execution environment and resource leases
- budget ledgers and exhaustion state
- communication route enforcement
- owner-scoped immutable Memento Fragments
- artifact and evidence metadata gates
- review independence, verification independence, and final evidence-map checks
- provider stream sessions, usage accounting, override policy checks, and durable failover events
- deterministic CLI e2e task execution that writes a workspace output file and closes with evidence, artifact, replay, and final submission
- Agent Thread Runtime loop that consumes provider-neutral model actions, records provider stream events, executes tool proposals through Tool Broker, auto-commits patch artifacts from diff evidence, blocks nonzero process checks, submits evidence-backed final output, checkpoints, and replays
- external process model adapter for `ModelTurnRequest` JSON over stdin and `ModelTurnResponse` JSON over stdout, so real provider wrappers can drive the same runtime without linking provider SDKs into the kernel
- a converged v0.1 Host OS tool target of `read_file`, `write_file`, `replace_text`, `delete_file`, and `run_command`
- Agent-OS control-plane tools for objective/checklist state, Supervisor communication, scoped blackboard posts, human asks, evidence records, supervised child agents, and final session submission
- a Supervisor-led workflow model where concrete distributions provide prompts, examples, and policy packs instead of hard-coded kernel pipelines
- Supervisor hierarchy semantics: the top Supervisor is `S0`, delegated Supervisors increment the level (`S1`, `S2`, ...), and every delegation records a durable invocation edge for replay and audit
- a runtime handle that can start turns, steer, interrupt, checkpoint, and expose status

## Core Surface

Agent-OS v0.1 separates three surfaces that must not be conflated:

```text
Host OS tools:
  read_file
  write_file
  replace_text
  delete_file
  run_command

Agent-OS control-plane tools:
  set_objective, update_checklist, record_evidence
  report_supervisor, post_blackboard, ask_human
  agent_control(action=start|status|output|set_hook|send|resume|stop|set_timeout|export_trace)
  submit_final

privileged agent_control actions:
  kill, delete_session, purge_state

distribution workflow:
  Supervisor-authored prompts, examples, policy packs, and optional role aliases
```

The core roles are `SupervisorAgent`, `WorkerAgent`, and `ReviewerAgent`. Testing is a worker responsibility that produces command evidence. Verification is primarily a kernel gate over final submissions and evidence maps; a distribution may add a verifier-style worker, but the kernel does not require one.

Supervisor levels are part of the control plane. `S0` owns the top-level goal. If a Supervisor delegates a sub-organization to another Supervisor, the child runs at `S1`; further delegation increments the level. The kernel records each delegation as an invocation edge with caller, callee, supervisor level, assignment, task/goal scope, and capability/profile snapshots.

Child agents are supervised, not fire-and-forget. `agent_control(action=start)` creates durable agent state and an invocation edge; the same call can set timeout, output policy, and initial hooks. `agent_control(action=set_hook)` can register periodic progress-report prompt injection so a child reports concise progress back to Supervisor without relying on done markers or a blocking wait tool.

## Live LLM Configuration

The interactive `chat` entrypoint supports both OpenAI-compatible and Anthropic-compatible calling while keeping Agent-OS tool execution provider-neutral.

```sh
LLM_BASE_URL=http://model.mify.ai.srv/v1
LLM_MODEL=tongyi/qwen3.6-plus
LLM_API_KEY=...
```

```sh
LLM_BASE_URL=http://model.mify.ai.srv/anthropic
LLM_MODEL=tongyi/qwen3.6-plus
LLM_API_KEY=...
```

`LLM_API_STYLE=openai-compatible|anthropic-compatible` may be set explicitly. If omitted, `/anthropic` base URLs use Anthropic-compatible Messages calling and other base URLs default to OpenAI-compatible Chat Completions calling. Existing `OPENAI_API_KEY`, `OPENAI_API_BASE`, and `AGENT_OS_MODEL` environment variables remain supported.

Run the current conformance suite with:

```sh
cargo test --workspace
```

Run the live goal-driven LLM coverage suite with real provider responses:

```sh
LLM_BASE_URL=http://model.mify.ai.srv/v1 \
LLM_MODEL=tongyi/qwen3.6-plus \
LLM_API_KEY=... \
cargo test -p agent-os-thread live_openai_compatible_llm_goal_driven -- --ignored --nocapture
```

```sh
LLM_BASE_URL=http://model.mify.ai.srv/anthropic \
LLM_MODEL=tongyi/qwen3.6-plus \
LLM_API_KEY=... \
cargo test -p agent-os-thread live_anthropic_compatible_llm_goal_driven -- --ignored --nocapture
```

These ignored tests use the normal system prompt and normal Agent Thread Runtime loop. They do not mock model responses and do not inject per-tool forcing prompts. The workspace scenario covers `read_file`, `write_file`, `replace_text`, `delete_file`, `run_command`, and `submit_final`. The control-plane scenario covers `set_objective`, `update_checklist`, `record_evidence`, `report_supervisor`, `post_blackboard`, `ask_human`, `agent_control`, `read_file`, and `submit_final`.

Inspectable interaction logs are written under `target/agent-os-audit/`:

```text
live-openai-compatible-goal-workspace.jsonl
live-openai-compatible-goal-control-plane.jsonl
live-anthropic-compatible-goal-workspace.jsonl
live-anthropic-compatible-goal-control-plane.jsonl
```

The tests also generate `.pretty.json` siblings for prompt and message audit. Logs must not contain provider API keys.

Run a local end-to-end task through the CLI:

```sh
cargo run -p agent-os-cli -- run \
  --workspace . \
  --task "Write a task report" \
  --output agent-os-task-result.md \
  --state-db .agent-os/state.sqlite \
  --bundle-output agent-os-task-bundle.json
```

The command records the task through the kernel lifecycle, attaches a writable environment lease, writes the output file through Tool Broker, records diff and command evidence from tool invocations, commits a patch artifact, submits the final evidence map, verifies replay, and can export a selected task bundle for offline audit or conformance replay.

`--state-db` stores the append-only event log in SQLite and replays it into kernel projections when the CLI starts, so later invocations can inspect or continue from the same durable control-plane state.

Inspect persisted state:

```sh
cargo run -p agent-os-cli -- status \
  --state-db .agent-os/state.sqlite
```

Resume an interrupted Agent Thread with an external model action process:

```sh
cargo run -p agent-os-cli -- resume \
  --state-db .agent-os/state.sqlite \
  --thread-id thread_000000000000000a \
  --workspace . \
  --bundle-output agent-os-resumed-bundle.json \
  --model-command path/to/model-action-wrapper
```

`resume` replays the SQLite event log, recovers an incomplete running turn to a resumable boundary, hydrates prior tool results and patch artifacts into the next `ModelTurnRequest`, and continues the same Agent Thread through the normal runtime loop.

Use an external model action process with the same runtime:

```sh
cargo run -p agent-os-cli -- run \
  --workspace . \
  --task "Write a task report" \
  --output agent-os-task-result.md \
  --state-db .agent-os/state.sqlite \
  --bundle-output agent-os-task-bundle.json \
  --model-command path/to/model-action-wrapper \
  --model-arg --profile \
  --model-arg local
```

The model command is executed once per turn. It receives compact `ModelTurnRequest` JSON on stdin and must emit `ModelTurnResponse` JSON on stdout, using actions such as `output_text`, `tool_call`, and `final`. Tool calls still execute only through the Agent-OS Tool Broker with capability checks, evidence capture, artifact commit, and replay.

Run a repository edit and test command through the same control plane:

```sh
cargo run -p agent-os-cli -- code \
  --workspace . \
  --task "Change answer from one to two" \
  --state-db .agent-os/state.sqlite \
  --bundle-output agent-os-code-bundle.json \
  --test-program cargo \
  --test-arg test
```

The `code` command is an implementation helper for a software-engineering workflow example. In task-only mode it derives one safe exact edit from phrasing like `from X to Y`; if it cannot prove a single edit, it fails closed and requires `--file`, `--old`, and `--new`. This helper demonstrates the Agent-OS control plane, but the kernel design target is Supervisor-led workflow prompts and examples rather than a mandatory built-in pipeline. Failed tests block completion; review findings can trigger a revision pass. `--bundle-output` exports the task subtree, including child tasks, artifacts, evidence, reviews, verifications, finals, profile snapshots, and replay events.

Exact edit mode is still available:

```sh
cargo run -p agent-os-cli -- code \
  --workspace . \
  --task "Change one exact snippet and run tests" \
  --file src/lib.rs \
  --old "old text" \
  --new "new text" \
  --test-program cargo \
  --test-arg test
```

## Documentation Map

Read in this order:

1. [Agent-OS Manifesto](docs/00-foundation/agent-os-manifesto.md)
2. [Agent Collaboration Theory](docs/00-foundation/agent-collaboration-theory.md)
3. [Documentation Index](docs/README.md)
4. [Architecture Principles](docs/00-foundation/architecture-principles.md)
5. [System Architecture](docs/10-kernel-design/system-architecture.md)
6. [Overall Architecture Mermaid](docs/10-kernel-design/overall-architecture-mermaid.md)
7. [Agent Thread Source Study](docs/05-research/agent-thread-source-study.md)
8. [Agent Thread Runtime](docs/10-kernel-design/agent-thread-runtime.md)
9. [Agent Thread Core Module](docs/10-kernel-design/agent-thread-core-module.md)
10. [Role and Profile System](docs/10-kernel-design/role-and-profile-system.md)
11. [Execution Environment System](docs/10-kernel-design/execution-environment-system.md)
12. [Scheduler and Resource Arbitration](docs/10-kernel-design/scheduler-and-resource-arbitration.md)
13. [Provider System](docs/10-kernel-design/provider-system.md)
14. [Agent Thread Communication](docs/10-kernel-design/agent-thread-communication.md)
15. [Memento Fragments](docs/10-kernel-design/memento-fragments.md)
16. [Kernel Data Model](docs/10-kernel-design/kernel-data-model.md)
17. [Kernel ABI and Syscalls](docs/10-kernel-design/kernel-abi-and-syscalls.md)
18. [State, Storage, and Replay](docs/10-kernel-design/state-storage-and-replay.md)
19. [Permission, Tool, and Evidence Model](docs/10-kernel-design/permission-tool-evidence-model.md)
20. [Production Roadmap](docs/20-implementation/production-roadmap.md)
21. [Conformance and Quality Gates](docs/20-implementation/conformance-and-quality.md)

Architecture decision records:

- [ADR-0001: Agent-OS is a microkernel-style runtime](docs/30-decisions/ADR-0001-agent-os-is-a-microkernel-runtime.md)
- [ADR-0002: PostgreSQL is a storage driver, not kernel state](docs/30-decisions/ADR-0002-postgresql-is-a-storage-driver.md)
- [ADR-0003: Agent Thread Runtime is proprietary core infrastructure](docs/30-decisions/ADR-0003-agent-thread-runtime-is-proprietary-core.md)
- [ADR-0004: Agent Thread Core Module is source-informed and clean-room](docs/30-decisions/ADR-0004-agent-thread-core-module-source-informed-clean-room.md)
- [ADR-0005: Memento Fragments are owner self-reminders](docs/30-decisions/ADR-0005-memento-fragments-are-owner-self-reminders.md)
- [ADR-0006: Agent Thread communication is capability-scoped](docs/30-decisions/ADR-0006-agent-thread-communication-is-capability-scoped.md)
- [ADR-0007: Provider System is global control-plane infrastructure](docs/30-decisions/ADR-0007-provider-system-is-global-control-plane.md)
- [ADR-0008: Thread-adjacent concerns are kernel subsystems](docs/30-decisions/ADR-0008-thread-adjacent-concerns-are-kernel-subsystems.md)

## Development Rules

The following rules are binding for implementation:

1. Kernel state MUST be derived from typed events and durable stores, not from chat history.
2. Agent Threads MUST interact with kernel state only through kernel-defined syscalls.
3. Tool execution MUST go through Tool Broker and Permission Kernel.
4. Every high-impact claim in final output MUST be backed by evidence.
5. Producer and Reviewer responsibilities MUST remain separated; verification gates MUST be enforced below prompts.
6. PostgreSQL, NATS JetStream, object storage, MCP, and UI consoles are drivers or distribution services, not kernel essence.
7. Third-party open-source agents MAY run as guests, but MUST NOT define the Agent-OS execution model.

## First Production Target

The first production-grade distribution should be the software engineering distribution because it has hard evidence surfaces:

- files
- diffs
- tests
- build logs
- review comments
- benchmark outputs
- git history

If the software engineering distribution cannot enforce evidence, permissions, recovery, and review discipline, less structured domains will not be stable either.
