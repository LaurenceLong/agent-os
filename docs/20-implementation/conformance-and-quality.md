# Conformance and Quality Gates

Status: normative

Last updated: 2026-06-27

## 1. Purpose

Agent-OS needs conformance tests because the project goal is kernel-plus-distributions, not a single closed application.

Third-party distributions should be able to extend the system without redefining core semantics.

## 2. Conformance Areas

Initial conformance areas:

```text
ACB lifecycle
syscall validation
event log
state replay
role/profile resolution
Supervisor hierarchy and invocation edges
execution environment attachment
scheduler and resource arbitration
budget ledger enforcement
permission enforcement
communication profile enforcement
provider system routing
Memento Fragment immutability
tool broker mediation
artifact lifecycle
evidence attachment
review independence
verification gates
final answer contract
storage driver behavior
agent package manifest
distribution manifest
```

## 3. Kernel Conformance

Kernel conformance tests MUST verify:

- invalid ACB is rejected
- invalid state transition is rejected
- every mutating syscall emits event
- replay rebuilds projections
- invalid role or profile binding is rejected
- Supervisor delegation records the correct `S<N+1>` level
- every spawned or delegated Agent Thread has a replayable invocation edge
- environment attach and release emit durable events
- resource lease conflict resolution is deterministic
- budget exhaustion changes admission outcome according to policy
- syscall without capability is rejected
- communication without route capability is rejected
- high-risk syscall requires approval when policy says so
- event ordering is stable per aggregate
- audit log is append-only
- Memento Fragment lifecycle replays from events

## 4. Agent Thread Runtime Conformance

Runtime conformance tests MUST verify:

- Agent Thread reads ACB through kernel API
- Agent Thread yields at required boundaries
- tool calls route through Tool Broker
- context loads route through Context Manager
- messages route through Communication Kernel
- model streams route through Provider System
- artifacts route through Artifact Store
- evidence routes through Evidence Store
- role restrictions are enforced
- thread cannot self-upgrade role, permission, or sandbox profile
- thread cannot use a writable workspace without an attached writable environment
- child Agent Thread cannot read or mutate parent Memento Fragments
- triggered Memento Fragments project only to the owner thread
- worker cannot send to Supervisor unless profile allows it
- worker cannot post to blackboard unless profile allows it
- worker cannot message human unless profile allows it
- worker communication is limited to Supervisor, scoped blackboard, and human routes allowed by profile
- crash and resume preserve task state

## 5. Tool Driver Conformance

Tool driver tests MUST verify:

- input schema validation
- output schema validation
- risk declaration
- idempotency declaration
- audit event emission
- evidence capture when applicable
- secret redaction behavior
- failure semantics
- provider capability declaration for model-facing tools where applicable
- Host OS tool surface contains exactly `read_file`, `write_file`, `replace_text`, `delete_file`, and `run_command`
- Agent-OS control-plane tools are grouped by work state, communication, agent supervision, privileged administration, and session lifecycle
- `wait_agent` is absent from the core surface; child progress reporting is covered by `agent_control(action=set_hook)`
- privileged `agent_control` actions are hidden from normal WorkerAgent tool views

### 5.1 Current v0.1 Tool Coverage

The current repo includes both deterministic mock/adapter tests and ignored live
LLM e2e tests.

The live tests MUST use real provider responses. They must use the normal system
prompt and normal runtime loop; test fixtures may define the task goal and
workspace state, but MUST NOT add hidden prompts that force a specific per-tool
call sequence.

Current live goal-driven scenarios:

```text
OpenAI-compatible workspace:
  cargo test -p agent-os-thread live_openai_compatible_llm_goal_driven_workspace_e2e -- --ignored --nocapture
  expected coverage: read_file, write_file, replace_text, delete_file, run_command, submit_final

OpenAI-compatible control plane:
  cargo test -p agent-os-thread live_openai_compatible_llm_goal_driven_control_plane_e2e -- --ignored --nocapture
  expected coverage: set_objective, update_checklist, record_evidence, report_supervisor, post_blackboard, ask_human, agent_control, read_file, submit_final

Anthropic-compatible workspace:
  cargo test -p agent-os-thread live_anthropic_compatible_llm_goal_driven_workspace_e2e -- --ignored --nocapture
  expected coverage: read_file, write_file, replace_text, delete_file, run_command, submit_final

Anthropic-compatible control plane:
  cargo test -p agent-os-thread live_anthropic_compatible_llm_goal_driven_control_plane_e2e -- --ignored --nocapture
  expected coverage: set_objective, update_checklist, record_evidence, report_supervisor, post_blackboard, ask_human, agent_control, read_file, submit_final
```

Audit logs are emitted to:

```text
target/agent-os-audit/live-openai-compatible-goal-workspace.jsonl
target/agent-os-audit/live-openai-compatible-goal-control-plane.jsonl
target/agent-os-audit/live-anthropic-compatible-goal-workspace.jsonl
target/agent-os-audit/live-anthropic-compatible-goal-control-plane.jsonl
```

Each log should contain the generated system prompt, provider request messages,
provider responses, tool invocations, tool results, and a
`live_goal_driven_summary` record. The summary coverage rate MUST be `6/6` for
workspace scenarios and `9/9` for control-plane scenarios. Pretty JSON siblings
may be generated for review, but secrets must remain redacted or absent.

## 6. Storage Driver Conformance

Storage driver tests MUST verify:

- append-only event semantics
- transactional projection update where required
- idempotent syscall result lookup
- lock acquire and release
- lease expiration
- replay from persisted events
- migration version tracking

SQLite and PostgreSQL drivers MUST pass the same logical conformance suite.

## 7. Evidence and Final Answer Gates

The system MUST reject or mark incomplete:

- final answer without evidence map
- patch claim without diff evidence
- test-passed claim without test log
- review claim without reviewed artifact version
- verification result based on stale artifact
- memory write without provenance
- writable mutation without attached environment or lease
- profile self-upgrade attempt from inside Agent Thread
- budget-exhausted work admitted without policy override
- forbidden provider override
- provider fallback without durable event
- blackboard post without allowed channel or scope
- human message without human communication route
- Memento Fragment used as evidence without promotion
- child mutation attempt against parent Memento Fragment
- high-risk action without approval record

## 8. Reliability Targets

Initial production targets:

```text
single-node task replay: required
worker restart recovery: required before distributed mode
1000-event task replay: required before v0.1 release
10000-event task replay: required before v0.2 release
permission bypass known test cases: zero allowed
final evidence coverage: measurable
audit export: required before production distro
```

## 9. Security Tests

Required test families:

- prompt injection against tool selection
- prompt injection against role or profile self-upgrade
- prompt injection against provider override
- prompt injection against memory write
- prompt injection against communication route widening
- prompt injection attempting sandbox or environment escape
- prompt injection against budget ledger state
- prompt injection attempting to expose or mutate parent Memento Fragments
- malicious MCP tool metadata
- shell command escalation
- path traversal
- capability token reuse
- stale approval reuse
- artifact tampering
- evidence tampering
- guest agent syscall bypass

## 10. Long-Task Benchmarks

The first long-task benchmark SHOULD use software engineering tasks.

Benchmark dimensions:

- number of events
- number of tool calls
- context compaction count
- number of artifacts
- number of review cycles
- recovery after interruption
- unsupported final claim count
- human approval count
- wall time
- token cost

The benchmark result MUST include task bundle export so failures can be replayed.
