# Production Roadmap

Status: planning baseline

Last updated: 2026-07-03

## 1. Strategy

Agent-OS should be built as production infrastructure from the start.

The correct sequence is:

```text
contracts -> single-node kernel -> Agent Thread Runtime -> tool/evidence loop -> multi-agent scheduling -> distributed control plane -> distributions -> ecosystem
```

The project SHOULD NOT start with a UI, a marketplace, or a generic workflow builder.

## Current Implementation Snapshot

The current repository is no longer only a planning artifact. It contains a
single-node Rust implementation of the v0.3.0 kernel, host, and Agent Thread
runtime:

- `agent-os-sys` owns ABI/data types.
- `agent-os-store` and `agent-os-store-sqlite` provide append-only event storage,
  blob storage, idempotency records, replay, and migration version tracking.
- `agent-os-kernel` owns lifecycle state, profiles, permissions, tool broker
  mediation, environments, leases, communication, blackboard entries, evidence,
  artifacts, review, verification, final submissions, and task bundle export.
- `agent-os-config` owns cross-platform Agent-OS roots, global
  `config.json`, project overrides, provider catalog resolution, last-good
  global config backup, and project runtime paths.
- `agent-os-ecosystem` discovers global and project rules, instructions,
  skills, commands, agents, and MCP declarations before they are imported into
  typed kernel state.
- The tool broker attaches managed text results to the unified tool-output
  manager. Long-running or large tool outputs can be read by `tool_call_id`
  using default `new` windows, explicit `head`/`tail` windows, or
  `full=true` plus `offset`/`limit` line paging over spooled fields.
- `agent-os-thread` owns the runtime loop, provider-neutral model actions,
  OpenAI-compatible and Anthropic-compatible adapters, prompt/message builders,
  parser logic, and live audit logs.
- `agent-os-distro` owns packaged distribution prompts, workflow labels,
  examples, and policy builders for the software-engineering distribution.
- `agent-os-app-server` owns the JSONL app protocol and typed projections for
  thread read/list/search/archive, fork, rollback, compact, paged turn and
  timeline reads, stats, notifications, automations, and task bundle export.
- `agent-os-host` opens the SQLite-backed kernel store, owns runtime job
  records and background workers, launches configured Agent Thread Runtime
  jobs, serves app projections, and recovers queued jobs after process restart.
- `agent-os-cli` provides run, code, chat, status, and resume entrypoints.
  `chat` starts `agent-os-hostd --stdio`, resolves the selected
  `provider/model` from the user's global config plus project overrides, and
  lets the host run provider-backed runtime jobs.
- Provider-backed runtime jobs consume typed model `limit`, `capabilities`, and
  model `options`; model `limit.output` supplies the default max-output token
  bound, provider-specific reasoning options are passed through explicitly, and
  LLM API failures are classified before they enter runtime feedback.
- `agent-os-conformance` captures the durable contract across lifecycle,
  security, communication, storage, provider routing, software distribution,
  runtime resume, and export behavior.
- `benchmarks/swe-bench-lite/private20_runner.py` contains the private
  SWE-bench Lite runner and official-harness evaluation workflow.

The current model-visible v0.3.0 tool surface is:

```text
Host OS:
  read_file
  read_image
  apply_patch
  run_command

Ecosystem:
  load_skill
  read_skill_resource

Work State:
  set_goal
  accomplish_goal
  update_checklist
  record_evidence

Communication:
  report_supervisor
  post_blackboard
  ask_human

Agent Supervision:
  agent_control

Session Lifecycle:
  submit_final
```

`agent_control` is one CLI-like tool with an `action` field. Normal actions are
`start`, `status`, `output`, `set_hook`, `send`, `resume`, `stop`,
`set_timeout`, and `export_trace`; privileged administration actions are
`kill`, `delete_session`, and `purge_state`. Permission-response actions are
`approve_permission` and `deny_permission`.

`agent_control(action=start)` uses `payload.goal` as the canonical child local
goal. `set_goal` is Supervisor-only retargeting for the Supervisor's own thread
or a direct child. `accomplish_goal` marks the caller's local goal accomplished
and closes active hooks; `submit_final` remains the session's final tool call.
`delete_session` and `purge_state` are replayable applied lifecycle commands
over an append-only event history.

OpenAI-compatible adapters serialize tool input objects as
`function.arguments`; Anthropic-compatible adapters send the same structured
objects as `tool_use.input`. The runtime, tool broker, evidence records, and
replay state stay provider-neutral.

The current validation baseline is:

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

AGENT_OS_LIVE_OPENAI_API_KEY=... \
AGENT_OS_LIVE_OPENAI_BASE_URL=https://api.openai.com/v1 \
AGENT_OS_LIVE_OPENAI_MODEL=gpt-4o \
cargo test -p agent-os-thread live_openai_compatible_llm_goal_driven -- --ignored --nocapture

AGENT_OS_LIVE_ANTHROPIC_API_KEY=... \
AGENT_OS_LIVE_ANTHROPIC_BASE_URL=https://api.anthropic.com \
AGENT_OS_LIVE_ANTHROPIC_MODEL=claude-sonnet-4-20250514 \
cargo test -p agent-os-thread live_anthropic_compatible_llm_goal_driven -- --ignored --nocapture
```

The benchmark validation baseline is intentionally stricter than Agent-OS
process completion: generated patches must be converted to predictions for the
exact evaluated instance ids, then scored with the official SWE-bench harness
from the WSL virtualenv documented in
`docs/20-implementation/swe-bench-lite-private-benchmark.md`.

## 2. Phase 0: Contracts

Goal: make the system implementable without ambiguity.

Deliverables:

- ATCB schema
- lifecycle state machine
- syscall schema
- event schema
- AgentOp schema
- AgentEvent schema
- AgentMessage envelope
- role profile schema
- permission profile schema
- sandbox profile schema
- communication profile schema
- communication delivery event schema
- execution environment schema
- environment lease schema
- scheduler policy schema
- resource lease schema
- budget ledger schema
- provider profile schema
- model alias schema
- routing policy schema
- blackboard schema
- artifact schema
- evidence schema
- Memento Fragment schema
- capability schema
- package manifest schema
- conformance test skeleton

Acceptance gates:

- mock agents can execute a complete lifecycle without LLMs
- mock agents can resolve an effective role, permission, sandbox, and scheduler binding
- events can rebuild task state
- invalid state transitions are rejected
- every schema is versioned

## 3. Phase 1: Single-Node Microkernel

Recommended stack:

```text
Rust
Tokio
SQLite
local filesystem artifact blobs
OpenTelemetry
```

Deliverables:

- `agent-os-kernel`
- `agent-os-sys`
- `agent-os-store`
- role/profile registry
- scheduler ready queue
- environment metadata store
- SQLite store driver
- event log
- ACB manager
- task DAG manager
- typed blackboard
- artifact metadata store
- evidence metadata store
- audit log

Acceptance gates:

- task can pause and resume
- state can replay from events
- environment and resource lease conflicts resolve deterministically
- artifact commit creates audit event
- evidence attach creates audit event
- no direct state mutation bypasses kernel API

## 4. Phase 2: Agent Thread Runtime

Goal: implement dedicated Agent Threads.

Normative module specification: [Agent Thread Core Module](../10-kernel-design/agent-thread-core-module.md).

Deliverables:

- deterministic outer loop
- effective role binding resolution
- Permission Profile and Sandbox Profile attachment
- Execution Environment attach flow
- Provider System contract
- provider profile resolution
- normalized stream event schema
- LLM adapter interface
- context request flow
- Communication Profile assignment
- kernel-routed message delivery
- Memento Fragment self-reminder flow
- syscall proposal flow
- yield checkpoint flow
- Supervisor hierarchy with `S0`, `S1`, `S2`, ... levels
- durable invocation edge recording for every delegation, producer assignment, review request, and human escalation
- built-in role runtimes:
  - SupervisorAgent
  - ReviewerAgent
  - ProducerAgent

Acceptance gates:

- ReviewerAgent cannot mutate reviewed workspace artifacts
- ProducerAgent cannot be the sole reviewer of its own artifact
- ProducerAgent can attach command logs through `run_command`
- SupervisorAgent cannot submit final without required evidence
- delegated Supervisors have recorded levels and invocation edges
- Agent Thread cannot widen its own role, permission, or sandbox profile
- Agent Thread executes only inside attached environments when environment policy applies
- Agent Thread can resume after process restart

## 5. Phase 3: Tool Broker and Permission Kernel

Goal: make tools behave like syscalls.

Deliverables:

- Tool Broker service
- Host OS model-visible tools:
  - `read_file`
  - `read_image`
  - `apply_patch`
  - `run_command`
- Agent-OS control-plane tool taxonomy:
  - work state: `set_goal`, `accomplish_goal`, `update_checklist`, `record_evidence`
  - communication: `report_supervisor`, `post_blackboard`, `ask_human`, `request_permissions`
  - agent supervision: `agent_control` with actions `start`, `status`, `output`, `set_hook`, `send`, `resume`, `stop`, `set_timeout`, `export_trace`, `approve_permission`, and `deny_permission`
  - privileged administration: `agent_control` actions `kill`, `delete_session`, and `purge_state`
  - session lifecycle: `submit_final`
- capability token model
- risk evaluation
- permission-profile intersection
- sandbox policy enforcement
- shell allowlist
- filesystem driver
- git driver
- MCP adapter
- approval flow
- idempotency handling

Acceptance gates:

- denied tool call is audited
- high-risk syscall can require approval
- file mutation requires scoped capability
- `wait_agent` is absent from the core tool surface
- tool result can become evidence
- repeated idempotent syscall returns consistent result

## 6. Phase 4: Evidence, Review, and Verification Loop

Goal: enforce production-quality task closure.

Deliverables:

- artifact lifecycle
- evidence map
- review request flow
- review finding schema
- verification request flow
- final submission contract
- unsupported-claim detector baseline

Acceptance gates:

- patch without diff evidence is rejected
- test claim without command log is rejected
- review of stale artifact version is rejected
- final answer without evidence map is rejected

## 7. Phase 5: Multi-Agent Scheduling and Isolation

Goal: make organization possible.

Deliverables:

- scheduler policy engine
- dependency scheduler
- Supervisor hierarchy scheduler inputs
- invocation graph traversal for cancellation, replay, and responsibility tracing
- resource locks
- resource lease arbiter
- environment pool and drain policy
- provider slot admission control
- file locks
- Communication Profile enforcement
- blackboard channel routing
- human attention queue
- blocked reason taxonomy
- stale context detector
- Memento Fragment trigger and projection policy
- loop detector
- conflict resolver
- budget ledgers and reservation flow

Acceptance gates:

- two ProducerAgents cannot mutate the same file concurrently
- thread without granted resource lease cannot consume exclusive resource
- ReviewerAgent reviews the exact artifact version
- ProducerAgent waits for required dependencies
- human interruption budget is enforced
- blocked agent records a machine-readable blocker
- scheduler can suspend low-value work

## 8. Phase 6: Distributed Control Plane

Recommended stack:

```text
PostgreSQL driver
NATS JetStream
object storage
remote workers
worker leases
distributed locks
```

Deliverables:

- PostgreSQL store driver
- NATS event bus adapter
- worker heartbeat
- worker lease manager
- remote environment backend
- object store driver
- distributed artifact blob references
- task bundle export

Acceptance gates:

- worker crash does not lose task state
- duplicate event delivery does not corrupt projection
- event replay works after restart
- artifact blob hash is verified
- remote worker cannot exceed capabilities

## 9. Phase 7: Official Software Engineering Distribution

Goal: prove production value in a hard-evidence domain.

Deliverables:

- software engineering distro manifest
- workflow prompts and examples for Supervisor-led software engineering work
- optional workflow step labels mapped onto core Producer or Reviewer semantics
- workspace filesystem policy using `read_file`/`read_image` plus `apply_patch` for one-file create, update, or delete operations
- command execution policy using `run_command`
- review policy pack
- final answer policy pack
- CLI entrypoint
- minimal console

Acceptance gates:

- real repository task is completed by Supervisor-driven workflow selection, not a hard-coded kernel pipeline
- final answer cites changed files, diff, tests, and risks
- failed tests block final acceptance
- review findings trigger revision

## 10. Phase 8: Ecosystem and Certification

Goal: allow others to build distributions on the kernel.

Deliverables:

- package registry format
- package signing
- conformance test suite
- conformance labels
- distro template
- SDKs for TypeScript, Python, and Rust

Acceptance gates:

- third-party agent package can be installed with declared capabilities
- incompatible package is rejected
- distribution can run conformance suite
- conformance report is machine-readable
