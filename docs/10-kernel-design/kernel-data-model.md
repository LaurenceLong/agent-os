# Kernel Data Model

Status: normative

Last updated: 2026-06-26

## 1. Purpose

This document defines the first production data model for Agent-OS kernel state.

The model is logical. SQLite, PostgreSQL, and future storage drivers may implement it differently, but they MUST preserve the same semantics.

## 2. Entity Relationship Overview

```text
Goal
  -> Task
      -> AgentControlBlock
      -> Artifact
          -> Evidence
          -> Review
              -> ReviewFinding
          -> Verification
      -> BlackboardEntry
      -> Approval
      -> ResourceLease
      -> Lock

AgentControlBlock
  -> AgentInvocation
  -> RoleProfile
  -> PermissionProfile
  -> SandboxProfile
  -> ExecutionEnvironment
  -> EnvironmentLease
  -> SchedulerPolicy
  -> BudgetLedger
  -> ProviderProfile
  -> ContextSnapshot
  -> CommunicationProfile
  -> AgentMessage
  -> MementoFragment
  -> CapabilityToken
  -> AuditEvent

MemoryRecord
  -> Evidence
  -> Approval

Event
  -> all aggregates
```

Every entity MUST be addressable by stable id.

Every entity that can influence final output MUST carry provenance.

## 3. Identifier Prefixes

Recommended id prefixes:

| Entity | Prefix |
|---|---|
| Goal | `goal_` |
| Task | `task_` |
| Agent | `agt_` |
| AgentThread | `thread_` |
| AgentInvocation | `inv_` |
| AgentTurn | `turn_` |
| Session | `sess_` |
| RoleProfile | `role_` |
| PermissionProfile | `perm_` |
| SandboxProfile | `sbox_` |
| ExecutionEnvironment | `env_` |
| EnvironmentLease | `envl_` |
| SchedulerPolicy | `sched_` |
| ResourceLease | `rlease_` |
| BudgetLedger | `bgt_` |
| ProviderProfile | `prov_` |
| ModelAlias | `alias_` |
| RoutingPolicy | `route_` |
| Message | `msg_` |
| Channel | `chan_` |
| CommunicationProfile | `comm_` |
| Context | `ctx_` |
| MementoFragment | `mmt_` |
| Memory | `mem_` |
| Capability | `cap_` |
| Artifact | `art_` |
| Evidence | `evd_` |
| Review | `rev_` |
| Verification | `ver_` |
| Approval | `apr_` |
| Lock | `lock_` |
| Event | `evt_` |
| Syscall | `sys_` |

IDs SHOULD be globally unique within a kernel namespace.

## 4. Goal

```yaml
goal_id: string
namespace: string
created_by: string
status: Registered | Active | Suspended | Completed | Failed | Cancelled
title: string
description: string
acceptance_criteria: string[]
constraints: string[]
risk_level: integer
deadline: string | null
root_task_id: string | null
created_at: string
updated_at: string
```

Rules:

- A goal MUST have acceptance criteria before execution begins.
- A goal cannot complete while required child tasks remain incomplete.
- A goal final output MUST reference accepted artifacts and evidence.

## 5. Task

```yaml
task_id: string
goal_id: string
parent_task_id: string | null
status: Created | Ready | Running | Blocked | Reviewing | Verifying | Completed | Failed | Cancelled
title: string
description: string
owner_agent_id: string | null
depends_on: string[]
blocks: string[]
required_artifact_types: string[]
required_evidence_types: string[]
blocked_reason: string | null
priority: integer
risk_level: integer
created_at: string
updated_at: string
```

Rules:

- A task can become `Ready` only when dependencies are satisfied.
- A task with required evidence cannot become `Completed` until evidence exists or a waiver is approved.
- Task dependency cycles MUST be rejected.

## 6. AgentControlBlock

The AgentControlBlock aggregate represents the Agent Thread Control Block (ATCB) for storage and replay.

The canonical v0.1 field-level schema is defined in [Agent Thread Core Module](agent-thread-core-module.md#6-agent-thread-control-block). [Agent Thread Runtime](agent-thread-runtime.md) defines the conceptual ACB foundation and lifecycle vocabulary, but it is not the canonical v0.1 field schema when the two documents differ.

Storage projections SHOULD index:

- `thread_id`
- `root_thread_id`
- `parent_thread_id`
- `invocation_id`
- `supervisor_level`
- `task.task_id`
- `task.goal_id`
- `role`
- `status`
- `config_snapshot.role_profile_id`
- `config_snapshot.permission_profile_id`
- `config_snapshot.sandbox_profile_id`
- `config_snapshot.communication_profile_id`
- `config_snapshot.provider_profile_id`
- `active_turn.turn_id`
- `recovery.last_checkpoint_id`

Rules:

- The kernel is the only writer of ATCB state.
- Agent Threads may propose state changes through syscalls.
- ATCB updates MUST emit events.
- The top-level Supervisor for a goal has `supervisor_level = 0`.
- Delegated Supervisors increment their caller Supervisor level by one.
- Worker and Reviewer threads have `supervisor_level = null`.
- Threads created by delegation, worker assignment, review request, or human escalation MUST reference an AgentInvocation.

## 6.1 AgentInvocation

AgentInvocation records the durable call edge between Agent Threads or between the system and a root Agent Thread.

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
capability_snapshot_id: string | null
profile_snapshot_id: string
status: Active | Superseded | Cancelled
created_at: string
superseded_by: string | null
```

Rules:

- Invocation edges are append-only.
- A root goal MUST have exactly one active `root_supervisor` edge to its `S0` Supervisor.
- A Supervisor delegated by `S<N>` MUST be recorded as `S<N+1>`.
- A caller cannot create a callee with a broader permission, sandbox, or communication profile than the kernel grants.
- Cancellation and replay traverse the invocation graph, not chat history.

## 7. BlackboardEntry

```yaml
entry_id: string
goal_id: string
task_id: string | null
section: goal | constraint | known_fact | hypothesis | decision | open_question | risk | test_result | review_result | acceptance_criterion
status: Active | Superseded | Invalidated | Resolved
content: object
confidence: number | null
source_evidence_ids: string[]
created_by_agent_id: string | null
created_at: string
superseded_by: string | null
```

Rules:

- Facts and decisions SHOULD have evidence.
- Hypotheses MUST NOT be promoted to facts without evidence.
- Supersession MUST preserve the old entry.

## 8. ContextSnapshot

```yaml
context_id: string
agent_id: string
task_id: string
loaded_refs: string[]
summary_artifact_id: string | null
freshness: Fresh | Stale | Unknown
pollution_score: number
token_estimate: integer
created_at: string
invalidated_at: string | null
```

Rules:

- Context snapshots are immutable.
- New context requires a new snapshot.
- Stale context MUST be visible to ReviewerAgent and verification gates.

## 9. RoleProfile

```yaml
role_profile_id: string
status: Active | Superseded | Revoked
name: string
role_family: producer | reviewer | operator | custom
purpose: string
default_permission_profile_id: string
default_sandbox_profile_id: string
default_provider_profile_id: string | null
default_scheduler_policy_id: string | null
allowed_child_role_profile_ids: string[]
required_review_mode: none | independent | dual
escalation_policy: object | null
distro_scope: core | distribution
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Role Profile labels do not grant authority by themselves.
- Distribution-defined roles MUST declare a core conformance family.
- Active turns consume a resolved binding snapshot, not live profile mutation.

## 10. PermissionProfile

```yaml
permission_profile_id: string
status: Active | Superseded | Revoked
name: string
max_risk_level: integer
allowed_syscalls: string[]
resource_scopes: string[]
denied_tool_classes: string[]
approval_required_above: integer
requires_evidence_for: string[]
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Permission ceilings MUST be enforced before capability grant or approval reuse.
- Capability tokens MUST NOT exceed the bound Permission Profile.

## 11. SandboxProfile

```yaml
sandbox_profile_id: string
status: Active | Superseded | Revoked
name: string
filesystem_mode: read_only | workspace_write | isolated_worktree | temp_only | custom
network_mode: off | allowlist | full
process_backend: native | job_object | container | vm | remote_worker
secret_policy: none | scoped_handles | injected_ephemeral
toolchain_profile_id: string | null
mount_policy: object | null
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Sandbox Profile defines the execution envelope, not the task intent.
- Writable mutation requires both capability scope and compatible sandbox policy.

## 12. ExecutionEnvironment

```yaml
environment_id: string
status: Requested | Provisioning | Ready | Attached | Draining | Terminated | Failed
backend_type: local_process | isolated_worktree | container | vm | remote_worker
template_name: string
sandbox_profile_id: string
host_id: string | null
workspace_mounts: object[]
artifact_mounts: object[]
toolchain_profile_id: string | null
network_policy_id: string | null
secret_projection_id: string | null
reuse_policy: exclusive | task_scoped | pooled
created_at: string
updated_at: string
terminated_at: string | null
```

Rules:

- Environment identity MUST be visible to audit and replay.
- Material backend or mount changes require a new event and, where necessary, a new environment instance.

## 13. EnvironmentLease

```yaml
environment_lease_id: string
environment_id: string
agent_id: string
thread_id: string
task_id: string
attach_mode: read_only | workspace_write | exclusive
status: Active | Released | Expired | Revoked
started_at: string
expires_at: string | null
released_at: string | null
```

Rules:

- Agent Threads execute only through attached environments when environment policy applies.
- Exclusive environment leases block conflicting concurrent use.

## 14. SchedulerPolicy

```yaml
scheduler_policy_id: string
status: Active | Superseded | Revoked
name: string
queue_class: foreground | background | review | verify | human_wait | batch
priority: integer
max_concurrent_children: integer
max_inflight_model_calls: integer | null
yield_policy: object | null
retry_policy: object | null
backoff_policy: object | null
starvation_window_ms: integer | null
budget_reservation_policy: object | null
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Scheduler Policy shapes admission and dispatch but does not bypass task dependency rules.
- Policy supersession MUST remain auditable.

## 15. ResourceLease

```yaml
resource_lease_id: string
resource_type: file | workspace | environment | provider_slot | blackboard_channel | artifact | deployment_target | memory_namespace | human_attention
resource_id: string
owner_agent_id: string
thread_id: string
goal_id: string
task_id: string
mode: shared | exclusive
status: Requested | Granted | Released | Expired | Denied
reason: string | null
lease_expires_at: string | null
created_at: string
released_at: string | null
```

Rules:

- Resource conflicts are resolved by the scheduler, not by thread-local retries alone.
- Denied resource leases remain auditable.
- Human attention and provider slots are first-class leaseable resources.

## 16. BudgetLedger

```yaml
budget_ledger_id: string
scope_type: goal | task | agent | provider_profile | human_attention
scope_id: string
status: Active | Exhausted | Suspended | Closed
token_limit: integer | null
tool_call_limit: integer | null
wall_time_limit_ms: integer | null
cost_limit: number | null
human_interrupt_limit: integer | null
model_request_limit: integer | null
tokens_used: integer
tool_calls_used: integer
wall_time_used_ms: integer
cost_used: number
human_interrupts_used: integer
model_requests_used: integer
reserved: object | null
reset_policy: string | null
created_at: string
updated_at: string
```

Rules:

- Budget is durable control-plane state, not only telemetry.
- Reservation SHOULD happen before materially expensive operations where policy requires it.
- Exhaustion affects future admission, not historical evidence.

## 17. ProviderProfile

```yaml
provider_profile_id: string
status: Active | Superseded | Revoked
name: string
default_provider_id: string | null
default_model_alias: string | null
routing_policy_id: string
fallback_chain: string[]
reasoning_defaults: object
tool_visibility_profile: string | null
timeout_ms: integer | null
max_output_tokens: integer | null
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Provider Profile is system-level configuration.
- Agent Threads bind to profiles; they do not inline provider SDK configuration.

## 18. ModelAlias

```yaml
model_alias_id: string
alias: string
provider_id: string
provider_model_name: string
capabilities: object
limits: object
cost: object
status: Active | Deprecated | Disabled
created_at: string
updated_at: string
```

Rules:

- Agent Threads should target aliases or policy, not raw provider model strings where possible.

## 19. RoutingPolicy

```yaml
routing_policy_id: string
status: Active | Superseded | Revoked
name: string
rules: object[]
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Routing Policy chooses effective provider/model based on role, task, environment, and override rules.
- Forbidden overrides must be rejected before stream open.

## 20. CommunicationProfile

```yaml
communication_profile_id: string
agent_id: string
thread_id: string
status: Active | Superseded | Revoked
supervisor:
  enabled: boolean
  allowed_message_types: string[]
  trigger_turn: boolean
  rate_limit: string | null
blackboard:
  enabled: boolean
  allowed_scopes: none | task | goal | global
  allowed_channels: string[]
  allowed_entry_types: string[]
  broadcast: boolean
  requires_review: boolean
human:
  enabled: boolean
  allowed_message_types: string[]
  requires_supervisor_approval: boolean
  attention_budget: integer | null
completion:
  required_report: boolean
  allowed_artifact_refs: boolean
  allowed_evidence_refs: boolean
created_at: string
updated_at: string
superseded_by: string | null
```

Rules:

- Communication Profile is assigned when the Agent Thread is created.
- The worker cannot widen its own Communication Profile.
- Profile changes require a kernel event and do not affect an active turn.

## 21. AgentMessage

```yaml
message_id: string
message_type: string
route: supervisor | blackboard | human
source_agent_id: string
source_thread_id: string
target_agent_id: string | null
target_thread_id: string | null
channel_id: string | null
goal_id: string
task_id: string
risk_level: integer
trigger_turn: boolean
requires_review: boolean
payload: object
artifact_refs: string[]
evidence_refs: string[]
delivery_status: Pending | Delivered | Rejected | Deferred | Expired
rejected_reason: string | null
created_at: string
delivered_at: string | null
```

Rules:

- Messages are kernel-routed.
- Rejected messages remain auditable.
- Human messages must respect attention budgets.
- Blackboard messages do not become facts until accepted by blackboard policy.

## 22. BlackboardChannel

```yaml
channel_id: string
scope: task | goal | global
name: string
allowed_entry_types: string[]
subscriber_agent_ids: string[]
requires_review: boolean
created_at: string
archived_at: string | null
```

Rules:

- Global channels require explicit communication capability.
- Channel subscribers receive events, not mutable shared text.

## 23. MementoFragment

```yaml
memento_id: string
owner_agent_id: string
owner_thread_id: string
goal_id: string
task_id: string
status: Draft | Armed | Triggered | Projected | Consumed | Superseded | Expired | Invalidated
anchor:
  anchor_type: child_thread_completed | tool_completed | approval_resolved | review_submitted | verification_submitted | turn_resumed | compaction_completed | time_reached | artifact_status_changed | manual
  anchor_ref: string | null
  condition: object | null
content:
  title: string
  body: string
  checklist: string[]
  structured: object | null
projection:
  mode: owner_context | owner_interrupt | owner_next_turn | supervisor_review
  priority: low | normal | high | critical
  max_projection_count: integer | null
immutability:
  content_hash: string
  committed_at: string | null
  committed_by: string | null
visibility:
  owner_only: true
  child_visible: false
links:
  related_child_thread_ids: string[]
  related_tool_call_ids: string[]
  related_artifact_ids: string[]
  related_evidence_ids: string[]
supersession:
  supersedes: string | null
  superseded_by: string | null
created_at: string
updated_at: string
expires_at: string | null
```

Rules:

- A Memento Fragment is an owner-scoped self-reminder.
- A child Agent Thread MUST NOT read or mutate a parent's Memento Fragment.
- `Draft -> Armed` freezes the content hash.
- Armed content is immutable for all actors, including the owner.
- The owner may supersede an armed fragment by creating a new fragment.
- Child completion may trigger a Memento Fragment, but cannot modify it.
- Memento Fragments are not durable memory, evidence, or blackboard facts.

## 24. CapabilityToken

```yaml
capability_id: string
agent_id: string
task_id: string
role: string
syscalls: string[]
resource_scopes: string[]
risk_ceiling: integer
expires_at: string | null
approval_id: string | null
created_at: string
revoked_at: string | null
```

Rules:

- Revoked or expired capabilities MUST be rejected.
- Capability use MUST be audited.
- High-risk capabilities SHOULD be short-lived.

## 25. Artifact

```yaml
artifact_id: string
goal_id: string
task_id: string
owner_agent_id: string
artifact_type: plan | patch | test_log | benchmark_result | review_report | analysis_note | final_answer | memory_proposal
version: integer
status: Draft | Submitted | UnderReview | ReviewFailed | NeedsRevision | Verified | Accepted | Rejected | Superseded | Archived
blob_ref: string | null
content_hash: string | null
metadata: object
created_at: string
updated_at: string
supersedes: string | null
```

Rules:

- Artifact versions are immutable after commit.
- New revisions create new versions or new artifacts.
- Patch artifacts MUST link to diff evidence.

## 26. Evidence

```yaml
evidence_id: string
goal_id: string
task_id: string | null
artifact_id: string | null
evidence_type: source_ref | diff_ref | command_log | test_result | benchmark_result | review_finding | approval_record | runtime_trace | screenshot | external_reference
producer_agent_id: string | null
claim: string | null
blob_ref: string | null
content_hash: string | null
metadata: object
status: Active | Invalidated | Superseded
created_at: string
invalidated_by: string | null
```

Rules:

- Evidence is immutable after commit.
- Invalid evidence remains in the audit trail.
- Evidence used by final output MUST be active and current.

## 27. Review and ReviewFinding

```yaml
review_id: string
artifact_id: string
artifact_version: integer
reviewer_agent_id: string
status: Requested | InProgress | Submitted | Accepted | Rejected | Superseded
focus: string[]
verdict: accept | reject | needs_revision
evidence_ids: string[]
created_at: string
submitted_at: string | null
```

```yaml
finding_id: string
review_id: string
severity: P0 | P1 | P2 | P3
title: string
body: string
location: object | null
evidence_ids: string[]
status: Open | Accepted | Rejected | Resolved
```

Rules:

- ReviewerAgent MUST NOT be the artifact owner.
- Review MUST specify artifact version.
- Findings remain durable even when rejected.

## 28. Verification

```yaml
verification_id: string
artifact_id: string | null
final_artifact_id: string | null
verifier_agent_id: string
status: Requested | Submitted | Failed | Passed
checked_claims: object[]
unsupported_claims: string[]
stale_evidence_ids: string[]
verdict: pass | fail | inconclusive
created_at: string
submitted_at: string | null
```

Rules:

- Final verification MUST check every high-impact final claim.
- Stale evidence MUST fail verification unless explicitly waived.

## 29. Approval

```yaml
approval_id: string
goal_id: string
task_id: string | null
requested_by_agent_id: string
approval_type: human | policy | external
scope:
  syscall_types: string[]
  resource_scopes: object[]
  risk_ceiling: integer
  goal_id: string
  task_id: string | null
risk_level: integer
status: Requested | Approved | Denied | Expired | Revoked
decision_by: string | null
decision_reason: string | null
created_at: string
decided_at: string | null
expires_at: string | null
```

Rules:

- Level 6 actions require human approval by default.
- Approval scope MUST be narrow.
- Approval reuse outside scope MUST be rejected.

## 30. Lock

```yaml
lock_id: string
resource_type: file | artifact | environment | provider_slot | memory_namespace | blackboard_channel | task | deployment_target | external_account | human_attention
resource_id: string
owner_agent_id: string
task_id: string
lease_expires_at: string
reason: string
risk_level: integer
status: Active | Released | Expired | ForceReleased
created_at: string
released_at: string | null
```

Rules:

- Concurrent mutation requires compatible locks.
- Force release MUST be audited.
- Expired locks may be reclaimed by the kernel.

## 31. MemoryRecord

```yaml
memory_id: string
namespace: string
status: Proposed | Active | Superseded | Invalidated
content: object
source_evidence_ids: string[]
created_by_agent_id: string | null
approved_by: string | null
created_at: string
activated_at: string | null
superseded_by: string | null
```

Rules:

- Memory writes require provenance.
- Proposed memory is not visible as authoritative memory.
- Invalidated memory must remain auditable.

## 32. AuditEvent

```yaml
audit_id: string
event_id: string
actor_type: human | agent | system
actor_id: string
action: string
resource_type: string
resource_id: string
reason: string | null
result: allow | deny | error | require_approval | success
created_at: string
```

Rules:

- Audit events are append-only.
- Permission decisions MUST create audit events.
- Tool invocations MUST create audit events.

## 33. Required Indexes

Logical indexes:

- events by aggregate id
- events by causation id
- tasks by goal and status
- threads by task and status
- role profiles by status and role family
- permission profiles by status
- sandbox profiles by status
- execution environments by status and backend type
- environment leases by agent, environment, and status
- scheduler policies by status and queue class
- resource leases by resource, owner, and status
- budget ledgers by scope and status
- provider profiles by status
- model aliases by alias and status
- routing policies by status
- messages by source, target, channel, and delivery status
- communication profiles by thread and status
- mementos by owner thread, anchor, and status
- artifacts by task and status
- evidence by artifact and status
- locks by resource and status
- approvals by status and expiration
- audit events by actor and resource

Storage drivers MAY add implementation-specific indexes.
