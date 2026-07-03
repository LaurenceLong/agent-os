# Conformance and Quality Gates

Status: normative

Last updated: 2026-07-04

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
runtime job queueing and background worker requeue
app-server projection and notification behavior
workspace crate dependency boundaries
artifact lifecycle
evidence attachment
review independence
verification gates
final answer contract
storage driver behavior
agent package manifest
distribution manifest
ecosystem import and replay
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
- ecosystem import events replay instructions, skills, commands, MCP servers,
  MCP tools, and imported agent profiles into identical projections

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
- producer cannot send to Supervisor unless profile allows it
- producer cannot post to blackboard unless profile allows it
- producer cannot message human unless profile allows it
- producer communication is limited to Supervisor, scoped blackboard, and human routes allowed by profile
- crash and resume preserve task state
- provider client calls have a hard request timeout and surface timeout failures
  through the normal provider retry/failure path
- host-backed runtime jobs do not duplicate live background workers and can
  requeue after foreground tool waits
- host-backed runtime jobs preserve `Blocked` as a terminal runtime job
  status instead of converting model non-finalization into host failure
- runtime model-context projection includes scoped context snapshots, context
  compactions, and active memory records in provider-visible prompt content
- runtime model-context projection includes owner-visible Memento Fragments
  through the kernel owner-only API, while draft, consumed, superseded, expired,
  and invalidated fragments stay out of normal context
- runtime model-context projection includes current-thread fork and rollback
  lifecycle boundaries so a live model can distinguish source, forked branch,
  and rollback reason from normal task text
- runtime context-pruning events remain explicit and replayable when context
  pressure supersedes older tool results, context snapshots, or memory records

## 5. Tool Driver Conformance

Tool driver tests MUST verify:

- input schema validation
- output schema validation
- risk declaration
- idempotency declaration
- descriptor lifecycle policy declaration, including foreground timeout,
  background continuation, managed output limits, and orphan-running recovery
- audit event emission
- evidence capture when applicable
- secret redaction behavior
- failure semantics
- provider capability declaration for model-facing tools where applicable
- Host OS tool surface contains exactly `glob_files`, `grep_files`, `read_file`, `read_image`, `apply_patch`, `run_command`, and `write_stdin`; `read_image` is visible only for image-capable model aliases, `glob_files` provides shell-free workspace path discovery by glob, `grep_files` provides shell-free UTF-8 content discovery by literal text, `apply_patch` covers workspace file creation, update, and deletion with exactly one file operation per call, and `write_stdin` continues kernel-owned piped process sessions by `process_id`
- Agent-OS control-plane tools are grouped by work state, communication, permission request, agent supervision, privileged administration, and session lifecycle
- `wait_agent` is absent from the core surface; child progress reporting is covered by `agent_control(action=set_hook)`
- model-visible tool projection comes from the kernel-owned `ToolPlan`; direct
  tools are filtered by effective permission set, S-level, model capability,
  and runtime planning mode, with hidden or disabled tools retaining typed plan
  entries and reasons
- `load_skill` and `read_skill_resource` enforce skill scopes and skill-root
  path containment
- local stdio MCP fixtures cover `tools/list`, dynamic tool registration,
  `tools/call`, schema validation, and permission denial
- every managed text tool-output field is queryable through the unified
  tool-output manager without inlining the full value into model context, and
  those output limits match descriptor lifecycle policy
- `tool_call_id` output lookup defaults to new output, supports bounded
  `head`/`tail` windows, and supports `full=true` with `offset`/`limit` line
  paging over the complete spooled field
- `run_command` creates a kernel-owned `ProcessSession`, returns `process_id`,
  records command mode, executed argv, lifecycle state, output cursors, and
  replayable exit or failure state
- process output chunks are appended as replayable sequence events, and
  `agent_control(action=output)` can poll by `process_id` plus
  `after_sequence`
- `run_command` supports explicit `stdin="piped"` process sessions, and
  `write_stdin` writes stdin by `process_id`, `write_id`, and `text` with
  replayable idempotent `ProcessStdinWritten` records; supervised stdin
  continues through `agent_control(action=send)`
- `agent_control(action=stop)` can interrupt a running process by
  `process_id`, `agent_control(action=kill)` can terminate a running process by
  `process_id`, and both lifecycle outcomes replay through `ProcessSession`
  state
- `agent_control(action=status)` can inspect one process by `process_id` or
  list target process sessions with `payload.processes=true` and optional
  lifecycle `state` filtering
- `thread/read` app-server projection includes kernel-owned
  `process_sessions` for the requested thread, and CLI `status --thread-id`
  relays that process lifecycle list
- app-server `process/stop` and `process/kill` requests route process cleanup
  through kernel-owned `ProcessSession` interrupt/terminate events, and CLI
  `process stop|kill --process-id` exposes the same cleanup path
- descriptor-declared evidence attachment for workspace, command, control-plane,
  and permission tools so final `evidence_map` entries can cite real evidence ids
- model-visible tool parameters are covered by semantic branch rather than only
  by one happy path, including target selection, optional pagination/cursor
  fields, process-specific variants, permission/risk failure, invalid input, and
  the returned fields the next model turn must consume

## 5.1 Model Context Coverage

The 60/25/15 unit/integration/live-e2e mix is measured by meaningful scenarios
and assertions. It is not a line-count target.

Every cross-boundary feature that can change model-visible context MUST have
integration coverage for the real path. This includes runtime-to-kernel tool
execution, kernel event and replay projection, store-backed restart behavior,
app-server and CLI orchestration, provider request construction, ecosystem and
config import into kernel state, and model-context projection.

Every operation that can change what the model sees on the next turn MUST be
reviewed as context-affecting. Examples include context load, compaction,
pruning, memento and memory projection, thread fork/rollback/resume, session
delete and purge, tool-result projection, imported instructions, skills,
commands, MCP tools/resources, provider tool visibility, and final-submission
evidence context. If a live model is expected to understand or invoke the
behavior, add goal-driven live LLM e2e coverage in addition to deterministic
unit or integration coverage.

## 5.2 Validation Gate Order

Changes that affect model-visible behavior, runtime behavior, tool behavior,
prompts, provider adapters, storage/replay, or benchmark behavior MUST pass
validation gates in this order:

1. Unit tests for the changed crates.
2. Integration and conformance tests.
3. Live LLM e2e tests through the normal runtime loop and provider adapters.
4. Private benchmark runs.

The private benchmark gate MUST NOT start until the live e2e gate has passed.
If a later gate fails, fix the root cause and restart from the earliest affected
gate instead of treating the failure as a benchmark result.

## 5.3 Ecosystem Conformance

Ecosystem conformance tests MUST verify:

- nearest project rule import and precedence
- global and project instruction imports do not bypass kernel events
- skill name/description extraction, Markdown fallback summaries, duplicate
  same-content coalescing, and duplicate different-content rejection
- command frontmatter import, `$1`/`$ARGUMENTS` expansion, and shell
  interpolation rejection
- imported agent Markdown to profile-seed projection
- runtime prompt projection lists available skills but does not inline unloaded
  skill bodies
- local stdio MCP `tools/list` registration and `tools/call` execution through
  the kernel tool broker
- denied skill and MCP calls record kernel authorization failures rather than
  executing drivers
- OpenAI-compatible and Anthropic-compatible model tool views project core and
  dynamic schemas from kernel `ToolDescriptor` records

## 5.4 Current v0.3 Tool and Live Coverage

The current repo includes both deterministic mock/adapter tests and ignored live
LLM e2e tests.

The live tests MUST use real provider responses. They must use the normal system
prompt and normal runtime loop; test fixtures may define the task goal and
workspace state, but MUST NOT add hidden prompts that force a specific per-tool
call sequence.

Live e2e commands resolve provider variables from the process environment first
and then from the repository-root `.env` file. Required variables:

```text
AGENT_OS_LIVE_OPENAI_API_KEY
AGENT_OS_LIVE_OPENAI_MODEL
AGENT_OS_LIVE_OPENAI_BASE_URL
AGENT_OS_LIVE_ANTHROPIC_API_KEY
AGENT_OS_LIVE_ANTHROPIC_MODEL
AGENT_OS_LIVE_ANTHROPIC_BASE_URL
```

Current live goal-driven scenarios:

```text
All ignored OpenAI/Anthropic live scenarios:
  cargo test -p agent-os-thread openai::tests::live -- --ignored --nocapture --test-threads=1
  expected coverage: simple file-writing e2e, workspace e2e, control-plane e2e, agent_control lifecycle e2e, full tool-surface e2e for OpenAI-compatible and Anthropic-compatible providers

OpenAI-compatible workspace:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven_workspace_e2e -- --ignored --nocapture
  expected coverage: read_file, apply_patch, run_command, accomplish_goal, submit_final

OpenAI-compatible control plane:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven_control_plane_e2e -- --ignored --nocapture
  expected coverage: set_goal, accomplish_goal, update_checklist, record_evidence, report_supervisor, post_blackboard, ask_human, request_permissions, agent_control, read_file, submit_final

Anthropic-compatible workspace:
  cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven_workspace_e2e -- --ignored --nocapture
  expected coverage: read_file, apply_patch, run_command, accomplish_goal, submit_final

Anthropic-compatible control plane:
  cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven_control_plane_e2e -- --ignored --nocapture
  expected coverage: set_goal, accomplish_goal, update_checklist, record_evidence, report_supervisor, post_blackboard, ask_human, request_permissions, agent_control, read_file, submit_final

Full tool surface:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven_full_tool_surface_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven_full_tool_surface_e2e -- --ignored --nocapture
  expected coverage: workspace tools, control-plane tools, agent_control action families, and parameter branches for read_file offset/limit, glob_files path/offset/limit, grep_files path/include/case_sensitive/offset/limit, and request_permissions scope plus complete PermissionSet fields

Ecosystem context:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven_ecosystem_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven_ecosystem_e2e -- --ignored --nocapture
  expected coverage: load_skill name/offset/limit, read_skill_resource name/path/offset/limit, tool_search discovery of deferred local stdio MCP tools, MCP tool call, and final submission evidence citations

Scoped context projection:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_goal_driven_scoped_context_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_anthropic_messages_llm_goal_driven_scoped_context_e2e -- --ignored --nocapture
  expected coverage: scoped context snapshots and context compactions projected into the normal provider prompt and used by the live model through apply_patch, run_command, and submit_final

OpenAI-compatible image input:
  cargo test -p agent-os-thread live_openai_chat_completions_llm_read_image_success_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_openai_chat_completions_llm_read_image_unsupported_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_openai_chat_completions_llm_switches_read_image_context_to_text_only_model -- --ignored --nocapture
  expected coverage: read_image success for image-capable aliases, hidden read_image for text-only aliases, and safe routing when prior image context is switched to a text-only alias

Anthropic-compatible image input:
  cargo test -p agent-os-thread live_anthropic_messages_llm_read_image_success_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_anthropic_messages_llm_read_image_unsupported_e2e -- --ignored --nocapture
  cargo test -p agent-os-thread live_anthropic_messages_llm_switches_read_image_context_to_text_only_model -- --ignored --nocapture
  expected coverage: read_image success for image-capable aliases, hidden read_image for text-only aliases, and safe routing when prior image context is switched to a text-only alias
```

Audit logs are emitted to:

```text
target/agent-os-audit/live-openai_chat_completions-goal-workspace.jsonl
target/agent-os-audit/live-openai_chat_completions-goal-control-plane.jsonl
target/agent-os-audit/live-openai_chat_completions-goal-full-tool-surface.jsonl
target/agent-os-audit/live-openai_chat_completions-goal-agent-control-lifecycle-success.jsonl
target/agent-os-audit/live-openai_chat_completions-goal-ecosystem.jsonl
target/agent-os-audit/live-openai_chat_completions-goal-scoped-context.jsonl
target/agent-os-audit/live-anthropic_messages-goal-workspace.jsonl
target/agent-os-audit/live-anthropic_messages-goal-control-plane.jsonl
target/agent-os-audit/live-anthropic_messages-goal-full-tool-surface.jsonl
target/agent-os-audit/live-anthropic_messages-goal-agent-control-lifecycle-success.jsonl
target/agent-os-audit/live-anthropic_messages-goal-ecosystem.jsonl
target/agent-os-audit/live-anthropic_messages-goal-scoped-context.jsonl
target/agent-os-audit/live-openai_chat_completions-read-image-success.jsonl
target/agent-os-audit/live-openai_chat_completions-read-image-unsupported.jsonl
target/agent-os-audit/live-openai_chat_completions-read-image-switch-text-only.jsonl
target/agent-os-audit/live-anthropic_messages-read-image-success.jsonl
target/agent-os-audit/live-anthropic_messages-read-image-unsupported.jsonl
target/agent-os-audit/live-anthropic_messages-read-image-switch-text-only.jsonl
```

Each log should contain the generated system prompt, provider request messages,
provider responses, tool invocations, tool results, and a
`live_goal_driven_summary` record. The summary coverage rate MUST be `6/6` for
workspace scenarios and `9/9` for control-plane scenarios. Pretty JSON siblings
may be generated for review, but secrets must remain redacted or absent.
Image-input logs contain `live_read_image_*` summary records and provider
message assertions for tool visibility and image payload projection. The
deterministic system-prompt export conformance additionally writes
`target/agent-os-audit/model-visible-context-review/` and asserts that scoped
context snapshots and context compactions appear in provider-visible prompt
content.

The 2026-06-30 long-running kernel refactor gate used the all-scenario command
above from WSL with exported provider variables. The observed result was 10
ignored-by-default live LLM e2e tests passing: five OpenAI-compatible scenarios
and five Anthropic-compatible scenarios. The v0.3 image-input gate adds six
ignored-by-default `read_image` live scenarios across OpenAI-compatible and
Anthropic-compatible adapter styles.

Host and app-server behavior is covered by focused `agent-os-host` tests
for JSONL app requests, runtime job persistence, configured provider workers,
background runtime workers, requeued jobs after background tool waits, runtime
job failure preservation, app projection resources, notifications, automation,
and task bundle export. The interactive `chat` command depends on this path:
`agent-os-cli` starts `agent-os-hostd --stdio`, sends typed app requests, and
lets the host launch configured Agent Thread Runtime workers from the user's
global provider config.

Workspace dependency boundaries are covered by a conformance test that runs
`cargo metadata --format-version=1 --no-deps` and checks normal `agent-os-*`
dependencies for production crates. The current contract keeps `agent-os-sys`
dependency-light, lets `agent-os-host` combine app-server/kernel/store/thread
components, keeps `agent-os-cli` on app-server/config/sys/distro only in
production, and keeps distribution prompt construction in `agent-os-distro`.

The private SWE-bench Lite gate is documented in
`docs/20-implementation/swe-bench-lite-private-benchmark.md`. Agent-OS process
exit code is not the benchmark pass condition; after patches are generated, the
official SWE-bench harness must evaluate the exact submitted instance ids and
report the intended resolved set.

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
- provider retry or stream failure without durable event
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
1000-event task replay: required before release hardening
10000-event task replay: required before distributed or benchmark-scale gates
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

Runtime quality gates also include model-loop feedback when behavior is visible
but not a hard authorization failure:

- repeated identical tool calls, including `read_file`, receive
  `runtime_feedback` instead of executing the third duplicate call
- after a bounded sequence of completed pre-patch read/command investigation
  tools without any `apply_patch` attempt, the runtime emits pre-patch
  resolution feedback while keeping the normal tool surface available
- if pre-patch investigation continues past the hard gate without an
  `apply_patch` attempt, the runtime narrows the projected tool surface to
  `apply_patch`, `submit_final`, and `accomplish_goal`
- while pre-patch resolution feedback is active, more investigation tool calls
  are rejected with `runtime_feedback`; the gate releases after any
  `apply_patch` attempt so normal patch-failure recovery can continue
- runtime feedback that actively controls tool-surface narrowing remains in the
  model projection after it falls outside the ordinary recent tool-result window
- once a thread has patch and command evidence, it receives finalization
  feedback telling the model to call `submit_final`
- while finalization feedback is active, the projected tool surface is narrowed
  to `submit_final` and `accomplish_goal`; any other tool call is rejected with
  `runtime_feedback`

Final-answer evidence checks are intentionally limited to concrete high-impact
claim families such as security, deletion, deployment, and migration. Local
test, finalization, and non-production workflow words are not high-impact
claims by themselves; otherwise normal benchmark closeout would be blocked by
its own validation language.

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
