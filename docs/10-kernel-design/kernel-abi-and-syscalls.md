# Kernel ABI and Syscalls

Status: normative

Last updated: 2026-06-26

## 1. Purpose

The kernel ABI defines how Agent Threads, workers, tools, storage drivers, policy engines, and distributions interact with Agent-OS.

The ABI MUST be typed, versioned, auditable, and stable enough for third-party distributions.

## 2. ABI Format

The initial ABI SHOULD be defined in:

- Protobuf for service contracts and event payloads
- JSON Schema for distribution manifests and policy packs
- Rust types in `agent-os-sys`

Every externally visible object MUST include:

- schema version
- stable identifier
- creation time
- provenance
- namespace or tenant scope where applicable

## 3. Syscall Envelope

All syscalls use a common envelope:

```json
{
  "abi_version": "0.1",
  "syscall_id": "sys_01HY...",
  "type": "tool.invoke",
  "agent_id": "agt_...",
  "task_id": "task_...",
  "session_id": "sess_...",
  "capability_token": "cap_...",
  "resource_scope": {},
  "risk_level": 3,
  "idempotency_key": "idem_...",
  "payload": {},
  "created_at": "2026-06-25T00:00:00Z"
}
```

The kernel MUST reject syscalls that lack identity, type, capability context, or task binding.

## 4. Kernel-Resolved Runtime Objects

Some critical runtime objects are not thread-authored payloads. They are kernel-resolved control-plane state that MUST still have stable ABI shape because runtimes, workers, policy engines, storage drivers, and distributions need to read them consistently.

Minimum object families:

```text
EffectiveBindingSnapshot
ExecutionEnvironmentDescriptor
EnvironmentLeaseDescriptor
SchedulerPolicyDescriptor
ResourceLeaseDescriptor
BudgetSnapshot
ApprovalDescriptor
AgentInvocationDescriptor
```

The authoritative logical fields for these objects are defined in [Kernel Data Model](kernel-data-model.md). The ABI fixes their transport shape and versioning requirements.

### 4.1 EffectiveBindingSnapshot

`agent.spawn` and later kernel lifecycle events MUST expose the effective binding under which the thread runs.

Minimum shape:

```yaml
role_profile_id: string
permission_profile_id: string
sandbox_profile_id: string
provider_profile_id: string | null
scheduler_policy_id: string | null
communication_profile_id: string
reasoning_profile: string | null
revision: integer
resolved_at: string
```

This snapshot is kernel-owned. The Agent Thread cannot widen it from inside a turn.

### 4.2 ApprovalDescriptor

Approval records exposed through the ABI MUST preserve bounded authorization scope.

Minimum shape:

```yaml
approval_id: string
scope:
  syscall_types: string[]
  resource_scopes: object[]
  risk_ceiling: integer
  goal_id: string
  task_id: string | null
status: Requested | Approved | Denied | Expired | Revoked
decision_by: string | null
expires_at: string | null
```

### 4.3 AgentInvocationDescriptor

Every Supervisor delegation, worker assignment, review request, human escalation, and root Supervisor creation MUST expose an invocation edge.

Minimum shape:

```yaml
invocation_id: string
goal_id: string
task_id: string
caller_thread_id: string | null
caller_agent_id: string | null
caller_supervisor_level: integer | null
callee_thread_id: string
callee_agent_id: string
callee_role_profile_id: string
callee_supervisor_level: integer | null
relationship: supervisor_delegation | worker_assignment | review_request | human_escalation | root_supervisor
assignment: string
profile_snapshot_id: string
created_at: string
```

## 5. Initial Syscall Set

### 5.1 Goal and Task

| Syscall | Purpose | Mutates State |
|---|---|---|
| `goal.register` | Register a top-level goal and acceptance criteria | yes |
| `task.spawn` | Create a task in the DAG | yes |
| `task.update` | Update task status, blocker, or metadata | yes |
| `task.block` | Mark task blocked with reason | yes |
| `task.complete` | Mark task complete with required artifacts and evidence | yes |

### 5.2 Agent Lifecycle

| Syscall | Purpose | Mutates State |
|---|---|---|
| `agent.spawn` | Create an Agent Thread and ACB | yes |
| `agent.yield` | Yield at a cooperative boundary | yes |
| `agent.suspend` | Request suspension | yes |
| `agent.resume` | Request resume | yes |
| `agent.fail` | Report failure with reason and evidence | yes |
| `agent.complete` | Complete agent assignment | yes |

`agent.spawn` result metadata SHOULD include:

- effective binding snapshot
- initial scheduler policy id
- attached or required environment ids
- communication profile id
- initial budget snapshot ids where applicable

### 5.3 Communication

| Syscall | Purpose | Mutates State |
|---|---|---|
| `comm.send_supervisor` | Send an allowed direct message to Supervisor | yes |
| `blackboard.post` | Post a typed message to an allowed blackboard scope or channel | yes |
| `human.message` | Request a human-facing message or question | yes |
| `comm.ack` | Acknowledge delivery or receipt of a message | yes |

Parent and peer direct routes are not v0.1 core syscalls. Parent authority is represented by the Supervisor that invoked the worker. Worker-to-worker coordination goes through Supervisor or scoped blackboard channels.

### 5.4 Context and Memory

| Syscall | Purpose | Mutates State |
|---|---|---|
| `context.load` | Request scoped context | yes |
| `context.commit_summary` | Commit a context summary | yes |
| `context.invalidate` | Mark context stale | yes |
| `memento.create` | Create an owner-scoped self-reminder draft | yes |
| `memento.arm` | Commit and freeze a Memento Fragment | yes |
| `memento.consume` | Mark an owner-visible reminder handled | yes |
| `memento.supersede` | Replace a reminder with a new immutable fragment | yes |
| `memento.invalidate` | Mark a reminder stale, unsafe, or incorrect | yes |
| `memory.query` | Query allowed memory namespaces | no |
| `memory.propose_write` | Propose durable memory write | yes |
| `memory.commit_write` | Commit approved memory write | yes |

### 5.5 Tools

| Syscall | Purpose | Mutates State |
|---|---|---|
| `tool.discover` | List visible tools | no |
| `tool.invoke` | Invoke a tool through Tool Broker | yes |
| `tool.cancel` | Cancel a running tool if supported | yes |
| `tool.attach_result` | Attach external tool result | yes |

Tool invocation admission MAY depend on:

- compatible environment attachment
- granted resource leases
- available budget
- approval state

### 5.6 Artifacts and Evidence

| Syscall | Purpose | Mutates State |
|---|---|---|
| `artifact.reserve` | Reserve an artifact id | yes |
| `artifact.commit` | Commit artifact metadata and blob reference | yes |
| `artifact.supersede` | Mark artifact replaced by newer version | yes |
| `evidence.attach` | Attach evidence to claim, task, artifact, or final | yes |
| `evidence.invalidate` | Mark evidence invalid or stale | yes |

### 5.7 Review and Verification

| Syscall | Purpose | Mutates State |
|---|---|---|
| `review.request` | Request independent review | yes |
| `review.submit` | Submit review finding | yes |
| `verify.request` | Request evidence verification | yes |
| `verify.submit` | Submit verification result | yes |
| `conflict.raise` | Raise conflict | yes |
| `conflict.resolve` | Resolve conflict with evidence | yes |

### 5.8 Policy and Human Approval

| Syscall | Purpose | Mutates State |
|---|---|---|
| `policy.check` | Check permission and risk | no |
| `approval.request` | Request human approval | yes |
| `approval.record` | Record approval decision | yes |

Approval records MUST be scoped to:

- requested action
- resource set
- risk level
- requesting agent and task
- expiration window

Human messaging and approval are related but not identical:

- `human.message` requests human-facing communication
- `approval.request` requests a decision record that may authorize later work

Both MAY consume human attention budget.

### 5.9 Final Output

| Syscall | Purpose | Mutates State |
|---|---|---|
| `final.draft` | Submit draft final answer | yes |
| `final.submit` | Submit final answer with evidence map | yes |
| `final.reject` | Reject final answer with reason | yes |

## 6. Event Envelope

All durable events use a common envelope:

```json
{
  "event_id": "evt_...",
  "event_type": "ArtifactCommitted",
  "abi_version": "0.1",
  "aggregate_type": "artifact",
  "aggregate_id": "art_...",
  "agent_id": "agt_...",
  "task_id": "task_...",
  "causation_id": "sys_...",
  "correlation_id": "goal_...",
  "payload": {},
  "created_at": "2026-06-25T00:00:00Z"
}
```

Events MUST be append-only.

Correction is represented by a new event, not mutation of old events.

Event families MUST include control-plane changes for:

- role and profile binding resolution
- environment provisioning and attachment
- resource lease grant, denial, and release
- budget reservation, debit, exhaustion, and override
- approval request, approval record, expiration, and revocation

## 7. Agent IPC Messages

Agent-to-agent communication MUST use structured messages routed through the kernel.

The ABI separates four layers that are easy to confuse:

| Layer | Purpose | Examples |
|---|---|---|
| Syscall | Capability-checked request into the kernel | `comm.send_supervisor`, `blackboard.post`, `human.message` |
| AgentOp | Submission-queue operation consumed by Agent Thread Runtime | `agent.spawn_child`, `comm.send_supervisor`, `blackboard.post`, `human.message` |
| Message type | Schema of the delivered payload | `StatusUpdate`, `CompletionReport`, `BlockerReport`, `FindingReport`, `Question`, `ReviewRequest`, `TestRequest`, `ArtifactSubmitted`, `EvidenceSubmitted`, `ApprovalRequest` |
| Durable event | Auditable result of routing and delivery | `CommunicationMessageSent`, `CommunicationMessageDelivered`, `CommunicationMessageRejected`, `BlackboardPostSubmitted`, `BlackboardPostPublished`, `HumanMessageRequested`, `HumanMessageDelivered` |

Communication mapping:

| Capability | Syscall / AgentOp | Message type examples | Durable events |
|---|---|---|---|
| Supervisor direct | `comm.send_supervisor` | `StatusUpdate`, `BlockerReport`, `RiskReport`, `CompletionReport` | `CommunicationMessageSent`, `CommunicationMessageDelivered`, `CommunicationMessageRejected` |
| Blackboard post | `blackboard.post` | `BlackboardPost` | `BlackboardPostSubmitted`, `BlackboardPostPublished`, `CommunicationMessageRejected` |
| Human route | `human.message` | `HumanQuestion`, `HumanEscalation`, `ApprovalRequest` | `HumanMessageRequested`, `HumanMessageDelivered`, `CommunicationMessageRejected` |

IPC messages MUST be persisted as events or event-linked records.

Uppercase names in foundation documents are conceptual labels only. They are not ABI identifiers unless a later schema explicitly registers them.

## 8. Package Manifest

Agent packages and distribution packages SHOULD include:

```yaml
manifest_version: "0.1"
package_name: string
package_type: agent | tool-driver | policy-pack | distro | memory-driver
version: string
entrypoint: string
required_kernel_version: string
capabilities_requested: string[]
roles_provided: string[]
tools_provided: string[]
schemas:
  - string
signature:
  algorithm: string
  value: string
```

Unsigned packages MAY be allowed in local development but SHOULD be rejected in production.

## 9. Compatibility Rules

Patch releases MUST NOT break existing syscalls.

Minor releases MAY add syscalls and optional fields.

Major releases MAY change ABI contracts, but MUST include migration guidance and conformance updates.

Distributions MUST declare the kernel ABI version they target.
