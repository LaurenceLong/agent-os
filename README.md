# Agent-OS

Agent-OS is a production-oriented runtime kernel for agent organizations.

It is not a chatbot framework, not a workflow engine, and not a thin wrapper around existing multi-agent projects. The system goal is to make agents observable, schedulable, recoverable, auditable, governable, and composable in the same way an operating system makes processes and threads manageable.

The central execution unit is the Agent Thread. An Agent Thread is not an arbitrary open-source agent loop. It is a first-class runtime entity controlled by the Agent-OS kernel through an Agent Control Block, scoped context, capability-checked syscalls, evidence requirements, artifacts, audit events, and cooperative scheduling boundaries.

## Design Baseline

Current baseline: `v0.3-design + Rust single-node host/runtime`

This repository contains the design contract and a Rust implementation that follows it. Implementation work should not start by adopting LangGraph, AutoGen, CrewAI, OpenHands, or any other agent framework as the core runtime. Those projects can be studied, wrapped, or hosted as compatibility guests, but the Agent-OS kernel and Agent Thread Runtime must be owned by this project.

## Rust Workspace

The implementation is contract-first and single-node:

```text
crates/
  agent-os-sys/          # ABI types, data model, syscalls, events
  agent-os-store/        # append-only store traits and in-memory driver
  agent-os-store-sqlite/ # SQLite event/idempotency store driver
  agent-os-kernel/       # single-node microkernel services
  agent-os-config/       # global config, project overrides, path resolution
  agent-os-ecosystem/    # global/project skills, rules, commands, MCP discovery
  agent-os-thread/       # Agent Thread Runtime loop and model adapters
  agent-os-distro/       # distribution prompt and workflow builders
  agent-os-app-server/   # JSONL app protocol and app-facing projection shape
  agent-os-host/         # thin host for persisted kernel state and runtime jobs
  agent-os-conformance/  # conformance tests for the normative docs
  agent-os-cli/          # run, code, chat, status, and resume entrypoints
```

Implemented kernel surfaces include:

- versioned syscall and event envelopes
- ABI/data schemas, including blackboard, context, memory, locks, package manifests, profiles, leases, artifacts, evidence, review, verification, and final submissions
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
- host-backed interactive `chat` execution through `agent-os-hostd --stdio`, the app-server JSONL protocol, SQLite-backed runtime job records, and user-level provider configuration
- Agent Thread Runtime loop that consumes provider-neutral model actions, records provider stream events, bounds provider requests with a hard client timeout, executes tool proposals through Tool Broker, yields on background-running tools, bounds pre-patch investigation loops, auto-commits patch artifacts from diff evidence, blocks nonzero process checks, submits evidence-backed final output through the broker, checkpoints, and replays
- external process model adapter for `ModelTurnRequest` JSON over stdin and `ModelTurnResponse` JSON over stdout, so real provider wrappers can drive the same runtime without linking provider SDKs into the kernel
- a converged v0.3 Host OS tool surface of `read_file`, `read_image`, `apply_patch`, and `run_command`, with each built-in tool owned by its own kernel Rust module and descriptor
- Agent-OS control-plane tools for objective/checklist state, Supervisor communication, scoped blackboard posts, human asks, evidence records, supervised child agents, and final session submission
- app-server-owned thread projections, including coarse `thread/read`, paged
  turn and timeline reads, branch fork records, rollback records, and manual
  compaction visibility, with host providing raw read models and runtime job
  state
- conformance coverage for workspace crate dependency boundaries so production crates cannot drift back into cross-layer dependencies
- a Supervisor-led workflow model where concrete distributions provide prompts, examples, and policy packs instead of hard-coded kernel pipelines
- Supervisor hierarchy semantics: the top Supervisor is `S0`, delegated Supervisors increment the level (`S1`, `S2`, ...), and every delegation records a durable invocation edge for replay and audit
- a runtime handle that can start turns, steer, interrupt, checkpoint, and expose status

## Core Surface

Agent-OS v0.3.0 separates these surfaces that must not be conflated:

```text
Host OS tools:
  read_file
  read_image
  apply_patch
  run_command

Ecosystem tools:
  load_skill, read_skill_resource

Agent-OS control-plane tools:
  set_goal, accomplish_goal, update_checklist, record_evidence
  report_supervisor, post_blackboard, ask_human, request_permissions
  agent_control(action=start|status|output|set_hook|send|resume|stop|set_timeout|export_trace|approve_permission|deny_permission)
  submit_final

privileged agent_control actions:
  kill, delete_session, purge_state

distribution workflow:
  Supervisor-authored prompts, examples, policy packs, and optional role aliases
```

The core roles are `SupervisorAgent`, `ProducerAgent`, and `ReviewerAgent`. Testing is a producer responsibility that produces command evidence. Verification is primarily a kernel gate over final submissions and evidence maps; a distribution may add a verifier-style workflow step, but the kernel does not require a dedicated verifier role.

Security levels are part of the control plane. Human authority is implicit `S0`; every persisted root agent starts at `S1`, and each nested child increments the parent level. The kernel records each delegation as an invocation edge with caller, callee, security level, child goal, task/goal scope, and capability/profile snapshots. `agent_control` and `set_goal` require `security_level <= 1` plus matching tool permission.

Child agents are supervised, not fire-and-forget. `agent_control(action=start)` creates durable agent state and an invocation edge with `payload.goal`; the same call can set timeout, output policy, success/failure criteria, initial hooks, and a child permission subset. `request_permissions` lets a child ask its direct parent for a turn- or session-scoped permission grant. `set_goal` is Supervisor-only retargeting for an existing Supervisor thread or direct child. Execution agents call `accomplish_goal` before `submit_final`; `submit_final` remains the last tool call in the session. `agent_control(action=set_hook)` can register periodic progress-report prompt injection so a child reports concise progress back to Supervisor without relying on done markers or a blocking wait tool.

Built-in tools define their own `ToolDescriptor`, model schema, runtime input policy, timeout policy, parameter parsing, execution entry, and focused unit tests under `crates/agent-os-kernel/src/tools/builtin/`. File-like outputs use `offset` and `limit` metadata where applicable. Long-running tool calls have a 15 second foreground wait cap; when work continues in the background, the invocation returns `Running` with `tool_call_id` equal to the tool call id. Every tool result is attached to the unified tool-output manager when it contains managed text fields. `agent_control(action=output, payload.tool_call_id=...)` reads that managed output: by default it returns `payload.new=200` lines from the supplied cursor; callers can request `payload.head` or `payload.tail`, or set `payload.full=true` with `payload.offset` and `payload.limit` for line paging over the complete spooled field. Hard byte caps still apply to each returned window. Tools that produce final-claim support attach evidence directly from their descriptor: file reads produce `source_ref`, patches produce `diff_ref`, commands produce `command_log`, control-plane state changes produce `runtime_trace`, and permission requests produce `approval_record`.

The runtime also narrows the model-visible tool surface when the loop has enough evidence to move forward. A pre-patch investigation gate first emits soft feedback after a bounded sequence of read/command investigation results without any `apply_patch` attempt, then temporarily exposes only `apply_patch`, `submit_final`, and `accomplish_goal` if investigation continues past the hard gate. After a patch has post-patch command evidence, the finalization gate exposes only `submit_final` and `accomplish_goal`.

## Provider Configuration

The interactive `chat` entrypoint reads provider configuration from the user's
global Agent-OS config, with optional project overrides from
`.agent-os/config.json`. Runtime state, blob storage, logs, provider audit
records, and caches default to global Agent-OS roots rather than the workspace.
Workspace `.agent-os` is reserved for user-maintained project configuration and
ecosystem resources.

Global config paths:

```text
Windows config: %APPDATA%\agent-os\config.json
Windows state/data/cache/log: %LOCALAPPDATA%\agent-os\...
macOS/Linux config: ${XDG_CONFIG_HOME:-$HOME/.config}/agent-os/config.json
macOS/Linux data:   ${XDG_DATA_HOME:-$HOME/.local/share}/agent-os
macOS/Linux state:  ${XDG_STATE_HOME:-$HOME/.local/state}/agent-os
macOS/Linux cache:  ${XDG_CACHE_HOME:-$HOME/.cache}/agent-os
```

Config shape:

```json
{
  "model": "openai/gpt-4o",
  "small_model": "openai/gpt-4o-mini",
  "provider": {
    "openai": {
      "api_key": "replace-with-your-api-key",
      "endpoint": "openai_chat_completions",
      "options": {
        "base_url": "https://api.openai.com/v1",
        "timeout_ms": 120000
      },
      "models": {
        "gpt-4o": {
          "name": "gpt-4o",
          "limit": {"context": 128000, "output": 16384},
          "capabilities": {
            "streaming": true,
            "tool_calling": true,
            "reasoning": true,
            "temperature": true,
            "image_input": true,
            "structured_output": true
          }
        },
        "gpt-4o-mini": {
          "name": "gpt-4o-mini",
          "limit": {"context": 128000, "output": 16384},
          "capabilities": {
            "streaming": true,
            "tool_calling": true,
            "reasoning": true,
            "temperature": true,
            "image_input": true,
            "structured_output": true
          }
        }
      }
    }
  }
}
```

`model` is the selected runtime model in `provider_id/model_id` form. Each
provider owns its credential, `endpoint`, endpoint options, and model catalog.
`endpoint` is required and supports the canonical values
`openai_chat_completions`, `openai_responses`, and `anthropic_messages`. Use
`agent-os chat --model <provider/model>` to select a non-default model from
the merged global/project config. Each model entry must explicitly define
`name`, `limit.context`, and `limit.output`; `limit.input` is optional.
Model `options` is an object merged into the provider request body before
runtime-controlled fields, so reasoning controls such as `reasoningEffort`,
`reasoningSummary`, or provider-native `thinking` settings belong there.
Project `.agent-os/config.json` may override `model`, `small_model`, provider
`options`, and model metadata, but it must not contain provider `api_key` or
`endpoint` values. Tests and isolated local runs may set `AGENT_OS_HOME` to
place all Agent-OS roots under one temporary directory.

Run the current conformance suite with:

```sh
cargo test --workspace
```

Run the live goal-driven LLM coverage suite with real provider responses:

The ignored live tests read provider values from the process environment first
and then from the repository-root `.env` file.

```sh
AGENT_OS_LIVE_OPENAI_API_KEY=... \
AGENT_OS_LIVE_OPENAI_BASE_URL=https://api.openai.com/v1 \
AGENT_OS_LIVE_OPENAI_MODEL=gpt-4o \
cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven -- --ignored --nocapture
```

```sh
AGENT_OS_LIVE_ANTHROPIC_API_KEY=... \
AGENT_OS_LIVE_ANTHROPIC_BASE_URL=https://api.anthropic.com \
AGENT_OS_LIVE_ANTHROPIC_MODEL=claude-sonnet-4-20250514 \
cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven -- --ignored --nocapture
```

These ignored tests use the normal system prompt and normal Agent Thread Runtime loop. They do not mock model responses and do not inject per-tool forcing prompts. The workspace scenario covers `read_file`, `apply_patch`, `run_command`, `accomplish_goal`, and `submit_final`. The control-plane scenario covers `set_goal`, `accomplish_goal`, `update_checklist`, `record_evidence`, `report_supervisor`, `post_blackboard`, `ask_human`, `agent_control`, `read_file`, and `submit_final`.

Inspectable interaction logs are written under `target/agent-os-audit/`:

```text
live-openai_chat_completions-goal-workspace.jsonl
live-openai_chat_completions-goal-control-plane.jsonl
live-anthropic_messages-goal-workspace.jsonl
live-anthropic_messages-goal-control-plane.jsonl
```

The tests also generate `.pretty.json` siblings for prompt and message audit. Logs must not contain provider API keys.

Run a local end-to-end task through the CLI:

```sh
cargo run -p agent-os-cli -- run \
  --workspace . \
  --task "Write a task report" \
  --output agent-os-task-result.md \
  --bundle-output agent-os-task-bundle.json
```

The command records the task through the kernel lifecycle, attaches a writable environment lease, writes the output file through Tool Broker, records diff and command evidence from tool invocations, commits a patch artifact, submits the final evidence map, verifies replay, and can export a selected task bundle for offline audit or conformance replay.

By default the CLI stores the append-only event log in the global Agent-OS state
database and replays it into kernel projections when the CLI starts, so later
invocations can inspect or continue from the same durable control-plane state.
`--state-db` remains available as an explicit test/debug override.

Inspect persisted state:

```sh
cargo run -p agent-os-cli -- status
```

Resume an interrupted Agent Thread with an external model action process:

```sh
cargo run -p agent-os-cli -- resume \
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
  --bundle-output agent-os-task-bundle.json \
  --model-command path/to/model-action-wrapper \
  --model-arg --profile \
  --model-arg local
```

The model command is executed once per turn. It receives compact `ModelTurnRequest` JSON on stdin and must emit `ModelTurnResponse` JSON on stdout, using actions such as `output_text` and `tool_call`. `submit_final` is a normal tool call and must be the last call in the session. Tool calls still execute only through the Agent-OS Tool Broker with capability checks, evidence capture, artifact commit, and replay.

Run a repository edit and test command through the same control plane:

```sh
cargo run -p agent-os-cli -- code \
  --workspace . \
  --task "Change answer from one to two" \
  --bundle-output agent-os-code-bundle.json \
  --test-program cargo \
  --test-arg test
```

The `code` command is an implementation helper that builds a software-engineering distro prompt and then runs the normal Agent Thread Runtime loop through the `agent-os-hostd` process. In task-only mode it derives one safe exact edit from phrasing like `from X to Y`; if it cannot prove a single edit, it fails closed and requires `--file`, `--old`, and `--new`. The distro prompt gives the Supervisor flexible workflow labels mapped onto core Agent-OS roles rather than a mandatory built-in pipeline. `--bundle-output` exports the task subtree, including child tasks, artifacts, evidence, reviews, verifications, finals, profile snapshots, and replay events.

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
7. [Agent Thread Runtime](docs/10-kernel-design/agent-thread-runtime.md)
8. [Agent Thread Core Module](docs/10-kernel-design/agent-thread-core-module.md)
9. [Role and Profile System](docs/10-kernel-design/role-and-profile-system.md)
10. [Execution Environment System](docs/10-kernel-design/execution-environment-system.md)
11. [Scheduler and Resource Arbitration](docs/10-kernel-design/scheduler-and-resource-arbitration.md)
12. [Provider System](docs/10-kernel-design/provider-system.md)
13. [Agent Thread Communication](docs/10-kernel-design/agent-thread-communication.md)
14. [Memento Fragments](docs/10-kernel-design/memento-fragments.md)
15. [Kernel Data Model](docs/10-kernel-design/kernel-data-model.md)
16. [Kernel ABI and Syscalls](docs/10-kernel-design/kernel-abi-and-syscalls.md)
17. [State, Storage, and Replay](docs/10-kernel-design/state-storage-and-replay.md)
18. [Permission, Tool, and Evidence Model](docs/10-kernel-design/permission-tool-evidence-model.md)
19. [Production Roadmap](docs/20-implementation/production-roadmap.md)
20. [Conformance and Quality Gates](docs/20-implementation/conformance-and-quality.md)
21. [SWE-bench Lite Private Benchmark](docs/20-implementation/swe-bench-lite-private-benchmark.md)

Architecture decision records:

- [ADR-0001: Agent-OS is a microkernel-style runtime](docs/30-decisions/ADR-0001-agent-os-is-a-microkernel-runtime.md)
- [ADR-0002: PostgreSQL is a storage driver, not kernel state](docs/30-decisions/ADR-0002-postgresql-is-a-storage-driver.md)
- [ADR-0003: Agent Thread Runtime is proprietary core infrastructure](docs/30-decisions/ADR-0003-agent-thread-runtime-is-proprietary-core.md)
- [ADR-0004: Agent Thread Core Module is native core infrastructure](docs/30-decisions/ADR-0004-agent-thread-core-module-source-informed-clean-room.md)
- [ADR-0005: Memento Fragments are owner self-reminders](docs/30-decisions/ADR-0005-memento-fragments-are-owner-self-reminders.md)
- [ADR-0006: Agent Thread communication is capability-scoped](docs/30-decisions/ADR-0006-agent-thread-communication-is-capability-scoped.md)
- [ADR-0007: Provider System is global control-plane infrastructure](docs/30-decisions/ADR-0007-provider-system-is-global-control-plane.md)
- [ADR-0008: Thread-adjacent concerns are kernel subsystems](docs/30-decisions/ADR-0008-thread-adjacent-concerns-are-kernel-subsystems.md)
- [ADR-0009: v0.1 core surface is minimal tools, supervised collaboration, and scoped blackboards](docs/30-decisions/ADR-0009-v0-1-core-surface-convergence.md)

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
