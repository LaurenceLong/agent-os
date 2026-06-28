# Agent Thread Communication

Status: normative

Last updated: 2026-06-26

## 1. Purpose

Agent Threads can communicate, but communication is not an ambient right.

Every Agent Thread receives a communication profile when it is created. The profile defines whether the thread may report to its Supervisor, which scoped blackboard channels it may post to, and whether it may contact a human.

This keeps worker agents useful without allowing noisy, unsafe, or authority-escalating communication.

## 2. Core Principle

Communication is a kernel-mediated capability.

An Agent Thread cannot directly write to another Agent Thread, the blackboard, a broadcast channel, or a human-facing surface. It must request a communication syscall. In v0.1, worker-to-worker coordination is Supervisor-routed. The Communication Kernel validates the request against:

- thread role
- task binding
- communication profile
- capability token
- target route
- channel policy
- risk level
- human attention budget
- message schema

## 3. Communication Profile

A Communication Profile is assigned at Agent Thread creation time.

Canonical logical fields are also listed in [Kernel Data Model](kernel-data-model.md#20-communicationprofile). The shape below is the same contract from the communication subsystem's point of view:

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

The profile is immutable for the lifetime of a turn. Changes require a kernel event and take effect on a later turn.

## 4. Creation-Time Decisions

When creating a worker Agent Thread, the Supervisor decides:

1. Can the worker report to its Supervisor?
2. Can the worker post to the blackboard?
3. Can the worker post only to task or goal channels, or also global channels?
4. Can the worker broadcast to subscribers?
5. Can the worker contact a human?
6. Which message types are allowed?
7. Which communication routes trigger a Supervisor turn?
8. Which routes require review before delivery?

These decisions are part of the worker's Agent Thread configuration snapshot.

## 5. Communication Routes

### 5.1 Supervisor Direct

Route: `supervisor`

Use cases:

- report blocker
- request decision
- request scope clarification
- report high-risk finding
- request escalation

Supervisor direct communication SHOULD be allowed for most workers, but rate-limited. Parent and Supervisor are the same authority concept in v0.1: a worker reports to the Supervisor that invoked it. The top-level Supervisor is `S0`; delegated Supervisors are `S1`, `S2`, and so on.

Direct worker-to-worker messaging is intentionally absent from the v0.1 core. If Worker A needs Worker B to do something, Worker A reports to Supervisor, and Supervisor decides whether to assign or wake Worker B. This prevents unbounded chat meshes and keeps responsibility in the invocation graph.

### 5.2 Blackboard Channel

Route: `blackboard`

Use cases:

- publish known fact candidate
- publish risk
- publish blocker
- publish test result
- publish review result
- publish decision candidate

Blackboard posts are shared state. They MUST be typed and provenance-carrying.

Blackboard scopes:

```text
global
goal
task
```

Channels:

```text
facts
risks
blockers
decisions
artifacts
evidence
questions
```

Global channels SHOULD be restricted to `S0` or explicitly trusted Supervisors.

### 5.3 Human Route

Route: `human`

Use cases:

- request clarification
- request approval
- report critical risk
- ask for missing credential or unavailable external context

Human communication consumes scarce attention. It SHOULD usually require Supervisor approval unless the worker was explicitly created with human communication rights.

Human route is not the same as approval route. Approval requests still go through Permission Kernel and the approval flow, but human messages may ask non-approval questions.

## 6. Message Types

Initial message types:

```text
StatusUpdate
CompletionReport
BlockerReport
FindingReport
RiskReport
Question
DecisionRequest
ClarificationRequest
ReviewRequest
TestRequest
ArtifactSubmitted
EvidenceSubmitted
ApprovalRequest
HumanQuestion
HumanEscalation
BlackboardPost
BroadcastAnnouncement
```

Every message type has a schema. Free-form prose may be a field, but it is never the whole message.

## 7. Message Envelope

```yaml
message_id: string
message_type: string
route: supervisor | blackboard | human
source_thread_id: string
source_agent_id: string
target_thread_id: string | null
target_agent_id: string | null
channel_id: string | null
task_id: string
goal_id: string
risk_level: integer
trigger_turn: boolean
requires_review: boolean
payload: object
artifact_refs: string[]
evidence_refs: string[]
causation_id: string | null
created_at: string
delivery:
  status: Pending | Delivered | Rejected | Deferred | Expired
  rejected_reason: string | null
```

## 8. Spawn Integration

The ABI syscall is `agent.spawn`. The Agent Thread submission queue may expose `agent.spawn_child` as the Supervisor operation that requests child creation. Both paths MUST support communication profile assignment.

Example:

```yaml
agent.spawn_child:
  role: WorkerAgent
  assignment: "Inspect replay invariants."
  communication_profile:
    supervisor:
      enabled: true
      allowed_message_types: [BlockerReport, RiskReport, CompletionReport]
      trigger_turn: false
      rate_limit: "5/hour"
    blackboard:
      enabled: true
      allowed_scopes: goal
      allowed_channels: [facts, risks]
      allowed_entry_types: [KnownFactCandidate, Risk]
      broadcast: false
      requires_review: false
    human:
      enabled: false
```

The worker cannot expand this profile by prompt, tool call, or blackboard post.

## 9. Delivery Semantics

Direct messages can either:

- enqueue into the receiver mailbox without starting a turn
- trigger a Supervisor turn when `trigger_turn` is allowed
- wait for Supervisor review before delivery
- be rejected by policy

Blackboard posts can either:

- update task-scoped blackboard projection
- update goal-scoped blackboard projection
- publish to a channel with subscribers
- wait for review before projection
- be rejected as unsupported or unsafe

Human messages can either:

- be routed immediately
- require Supervisor approval
- be converted into an approval request
- be deferred due to attention budget
- be rejected as outside worker authority

## 10. Blackboard Broadcast

Blackboard broadcast is channel-based, not raw global chat.

Rules:

- every post has a typed payload
- every post has source thread and task provenance
- broadcasts must declare audience scope
- global broadcast requires explicit capability
- subscribers receive events, not mutable shared text
- blackboard facts require promotion and evidence rules from the Typed Blackboard

## 11. Relationship to Memento Fragments

Memento Fragments are owner self-reminders.

Communication messages are external delivery objects.

Rules:

- a worker cannot read or mutate Supervisor Memento Fragments
- a child completion message may trigger a Supervisor's Memento Fragment
- the Memento content is not delivered to the child
- communication payloads do not become Memento Fragments automatically

## 12. Safety Rules

1. Communication cannot grant capabilities.
2. Communication cannot override role restrictions.
3. Blackboard posts are not facts until accepted by blackboard policy.
4. Human messages must respect human attention budgets.
5. Direct worker-to-worker messages are not a v0.1 core route.
6. Global broadcasts require explicit capability.
7. A worker cannot create new communication routes for itself.
8. Failed delivery must be visible to the sender as an event.
9. Rejected messages remain auditable.

## 13. Conformance Tests

Minimum tests:

1. Worker without Supervisor route cannot send Supervisor direct message.
2. Worker with Supervisor route can send only allowed message types.
3. Worker without blackboard route cannot post to blackboard.
4. Worker with task-scope blackboard route cannot post global broadcast.
5. Blackboard post carries source thread, task, and evidence provenance.
6. Worker without human route cannot message human.
7. Human route can require Supervisor approval.
8. Communication profile cannot be expanded by the worker.
9. Rejected message emits audit and delivery event.
10. Child completion message can trigger Supervisor Memento Fragment without exposing the Memento content.
