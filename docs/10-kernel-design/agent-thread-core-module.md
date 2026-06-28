# Agent Thread Core Module

Status: normative

Last updated: 2026-06-25

## 1. Purpose

Agent Thread is the first core module of Agent-OS.

It is the kernel-managed execution unit that turns goals, context, model reasoning, tool calls, artifacts, evidence, review, and verification into a recoverable production workflow.

An Agent Thread is not a prompt, not a workflow node, not a chatbot session, and not an imported agent framework. It is an operating-system-style runtime object with a kernel-owned control block, mailbox, event stream, lifecycle state, permissions, resource leases, context projection, and recovery contract.

This document refines [Agent Thread Runtime](agent-thread-runtime.md) into an implementable module specification.

## 2. Design Inputs

This module is informed by [Agent Thread Source Study](../05-research/agent-thread-source-study.md).

The source study is an input, not authority. The Agent-OS ABI defined here is authoritative.

Source boundary:

- OpenCode and OpenAI Codex public source can inform control-plane patterns.
- Public Claude Code documentation can inform product behavior patterns.
- Leaked or non-public Claude Code source MUST NOT be inspected, copied, or used as implementation input.

## 3. Core Axioms

1. Agent Thread state is kernel state.
2. Chat history is never canonical state.
3. Every external effect is a syscall.
4. Every model call belongs to exactly one Agent Turn.
5. Every Agent Turn has a configuration snapshot.
6. Every tool invocation has a lifecycle record.
7. Every high-impact final claim needs evidence.
8. A role can restrict an Agent Thread, but cannot grant authority beyond capabilities.
9. Hooks can restrict, observe, or request review, but cannot bypass the Permission Kernel.
10. Recovery must be designed before multi-agent scale.

## 4. Module Package Boundary

Initial implementation packages SHOULD be:

```text
crates/
  agent-os-thread/             # Agent Thread Runtime
  agent-os-thread-protocol/    # AgentOp, AgentEvent, AgentItem, schemas
  agent-os-agent-control/      # spawn, registry, capacity, hierarchy
  agent-os-communication/      # profiles, routes, channels, delivery
  agent-os-provider/           # provider profiles, routing, stream sessions
  agent-os-provider-adapters/  # provider-specific adapters
  agent-os-thread-store/       # thread event and checkpoint persistence
```

The packages may be colocated during early development, but their public APIs MUST remain separable.

The implementation language SHOULD be Rust with Tokio. TypeScript and Python SHOULD consume generated SDKs rather than define kernel behavior.

## 5. Object Model

### 5.1 Agent Thread

Long-lived durable execution object.

Owns:

- Agent Thread Control Block
- submission queue
- event queue
- active turn state
- visible context projection
- resource leases
- child Agent Thread metadata
- recovery checkpoint cursor

### 5.2 Agent Turn

One model/tool interaction loop inside an Agent Thread.

An Agent Turn starts from an Agent Op and ends in one of:

- completed
- interrupted
- failed
- blocked
- compacted and continued
- awaiting external input

### 5.3 Agent Step

One model request plus the tool batch and accounting that follow it.

A turn may contain multiple steps when the model asks for tools and then continues.

### 5.4 Agent Item

Typed item emitted during a turn.

Initial item types:

```text
UserMessage
SystemDirective
DeveloperDirective
ContextReference
ContextSummary
CommunicationMessage
BlackboardPost
HumanMessageRequest
MementoFragment
Plan
ReasoningSummary
AssistantMessage
ToolCall
ToolResult
CommandExecution
FileChange
Patch
Snapshot
ArtifactReference
EvidenceReference
ReviewFinding
VerificationResult
SubagentMessage
PermissionRequest
PermissionDecision
ContextCompaction
FinalDraft
FinalSubmission
Error
```

Model-visible chat is a projection from Agent Items, not the source of truth.

### 5.5 Tool Invocation

A kernel-mediated syscall execution record.

Every Tool Invocation has:

- stable call id
- tool name and version
- schema id
- normalized input
- risk level
- permission decision
- sandbox profile
- status
- output summary
- evidence refs
- audit refs

### 5.6 Evidence Record

Immutable support for a claim, artifact, review, verification, or final answer.

Evidence is linked to Agent Items and syscalls.

### 5.7 Memento Fragment

A Memento Fragment is an immutable owner-scoped self-reminder that an Agent Thread leaves for its future self.

Memento Fragments are defined in [Memento Fragments](memento-fragments.md).

They are commonly anchored to child completion, tool completion, approval resolution, compaction, review callbacks, or resume. They are projected back to the owner Agent Thread when triggered.

A child Agent Thread MUST NOT read or mutate a parent's Memento Fragment. Child-facing instructions belong in the child assignment payload, not in Memento Fragments.

### 5.8 Communication Profile

A Communication Profile defines which communication routes an Agent Thread may use.

Communication is defined in [Agent Thread Communication](agent-thread-communication.md).

The profile is assigned when an Agent Thread is created. It controls whether the thread may report to its Supervisor, post to scoped blackboard channels, or request human attention. The thread cannot widen its own communication profile.

## 6. Agent Thread Control Block

The Agent Thread Control Block extends the ACB in [Agent Thread Runtime](agent-thread-runtime.md).

Minimum v0.1 fields:

```yaml
thread_id: string
session_id: string
root_thread_id: string
parent_thread_id: string | null
invocation_id: string | null
supervisor_level: integer | null
agent_path: string
role: string
owner: string
status: ThreadStatus
status_reason: string | null

task:
  task_id: string
  goal_id: string
  local_goal: string
  success_criteria: string[]
  failure_criteria: string[]

config_snapshot:
  model_provider_id: string
  model_id: string
  provider_profile_id: string
  model_routing_policy_id: string
  provider_adapter_version: string
  role_profile_id: string
  communication_profile_id: string
  permission_profile_id: string
  sandbox_profile_id: string
  context_policy_id: string
  memory_policy_id: string
  tool_registry_snapshot_id: string
  workspace_roots: string[]
  environment_ids: string[]
  reasoning_profile: string | null

queues:
  submission_cursor: string | null
  event_sequence: integer
  mailbox_cursor: string | null

active_turn:
  turn_id: string | null
  status: TurnStatus | null
  active_step_id: string | null
  expected_turn_id: string | null
  model_turn_state_ref: string | null
  started_at: string | null

resources:
  held_locks: string[]
  workspace_isolation_ref: string | null
  sandbox_ref: string | null
  background_process_refs: string[]

budgets:
  token_budget: integer | null
  tool_call_budget: integer | null
  wall_time_budget_ms: integer | null
  cost_budget: number | null
  max_steps_per_turn: integer
  max_child_threads: integer

recovery:
  last_checkpoint_id: string | null
  replay_cursor: string | null
  last_materialized_event_sequence: integer
  dirty: boolean

audit:
  created_at: string
  updated_at: string
  created_by: string
  termination_reason: string | null
```

The ATCB is not writable by the Agent Thread. Updates occur only through kernel transition functions.

The fields `role_profile_id`, `permission_profile_id`, `sandbox_profile_id`, `environment_ids`, and scheduler-related budgets are resolved by dedicated kernel subsystems. The Agent Thread Runtime consumes them, but does not define their semantics locally. See [Role and Profile System](role-and-profile-system.md), [Execution Environment System](execution-environment-system.md), and [Scheduler and Resource Arbitration](scheduler-and-resource-arbitration.md).

## 7. State Machines

### 7.1 ThreadStatus

```text
Created
Ready
Running
WaitingTool
WaitingPermission
WaitingUser
Blocked
Suspended
ResidentIdle
Unloaded
Completing
Completed
Failed
Interrupted
Quarantined
Terminated
```

### 7.2 TurnStatus

```text
Pending
InProgress
AwaitingTool
AwaitingPermission
AwaitingUser
Compacting
Completed
Failed
Interrupted
Blocked
```

### 7.3 StepStatus

```text
Created
CallingModel
StreamingModel
DispatchingTools
WaitingTools
RecordingResults
CheckingCompaction
Completed
Failed
Cancelled
```

### 7.4 ToolCallStatus

```text
Proposed
Validated
PendingApproval
Denied
Running
Completed
Failed
Cancelled
TimedOut
```

All transitions MUST be validated by deterministic transition functions.

## 8. Agent IPC

Agent Thread IPC uses a submission queue and an event queue.

### 8.1 AgentOp Envelope

```json
{
  "abi_version": "0.1",
  "op_id": "op_...",
  "thread_id": "thread_...",
  "type": "turn.start",
  "expected_turn_id": null,
  "idempotency_key": "idem_...",
  "causation_id": "evt_...",
  "submitted_by": "user|kernel|agent|api",
  "created_at": "2026-06-25T00:00:00Z",
  "payload": {}
}
```

Initial AgentOp types:

```text
thread.configure
turn.start
turn.steer
turn.interrupt
turn.inject_items
agent.spawn_child
agent.send_message
comm.send_supervisor
blackboard.post
human.message
memento.create
memento.arm
memento.consume
memento.supersede
memento.invalidate
agent.request_review
agent.request_verification
agent.update_config
agent.grant_capability
agent.revoke_capability
agent.checkpoint
agent.suspend
agent.resume
agent.shutdown
```

### 8.2 AgentEvent Envelope

```json
{
  "abi_version": "0.1",
  "event_id": "evt_...",
  "thread_id": "thread_...",
  "turn_id": "turn_...",
  "sequence": 42,
  "event_type": "ToolCallCompleted",
  "causation_id": "op_...",
  "correlation_id": "goal_...",
  "created_at": "2026-06-25T00:00:00Z",
  "payload": {}
}
```

Initial AgentEvent types:

```text
ThreadConfigured
ThreadStatusChanged
TurnStarted
TurnSteered
TurnInterrupted
TurnCompleted
TurnFailed
TurnBlocked
StepStarted
StepCompleted
ItemStarted
ItemDelta
ItemCompleted
ToolCallProposed
ToolCallApprovalRequested
ToolCallApprovalResolved
ToolCallStarted
ToolCallCompleted
ToolCallFailed
PatchDetected
SnapshotCaptured
ContextCompactionRequested
ContextCompactionCompleted
EvidenceAttached
CommunicationMessageSent
CommunicationMessageDelivered
CommunicationMessageRejected
BlackboardPostSubmitted
BlackboardPostPublished
HumanMessageRequested
HumanMessageDelivered
MementoFragmentArmed
MementoFragmentTriggered
MementoFragmentProjected
MementoFragmentConsumed
MementoFragmentSuperseded
MementoFragmentInvalidated
ArtifactCommitted
ReviewRequested
ReviewSubmitted
VerificationRequested
VerificationSubmitted
ChildThreadSpawned
ChildThreadCompleted
CheckpointCommitted
ThreadUnloaded
ThreadResumed
```

Events are append-only. Corrections are new events.

## 9. Turn Execution Loop

Every Agent Turn MUST follow this loop:

```text
1. accept AgentOp if thread is schedulable
2. create TurnState and config snapshot
3. acquire required resource leases
4. project model-visible context from typed state
5. request Provider Stream Session
6. emit TurnStarted
7. create AgentStep
8. call model through Provider System / Model Gateway
9. stream model output into Agent Items
10. validate proposed tool calls
11. route tool calls through Tool Broker syscalls
12. record tool results and evidence
13. record token usage, cost, diff, and snapshot
14. check loop, budget, and compaction policy
15. continue, compact, block, fail, or complete
16. commit checkpoint
17. release turn-scoped resources
```

The model may propose actions, but the runtime decides whether the proposal becomes a syscall.

## 10. Turn Admission and Rejection

`turn.start` MUST be rejected with a machine-readable reason when the Agent Thread cannot start a new turn.

Initial rejection reasons:

```text
Busy
PendingTriggerTurn
AwaitingApproval
AwaitingUserInput
PlanModeRequiresExit
DependencyBlocked
ResourceLockUnavailable
OutOfBudget
ThreadSuspended
ThreadUnloaded
ThreadQuarantined
ThreadTerminated
```

`turn.steer` MUST include the expected active turn id. The kernel rejects stale steering.

## 11. Provider System

The Provider System is the system-level module that isolates provider-specific behavior and exposes a runtime-facing Model Gateway.

Required abstractions:

```text
ProviderProfile
ModelRoutingPolicy
ModelProvider
ModelCapability
ModelClient
ModelTurnSession
ModelStreamEvent
ProviderTransform
```

Rules:

- Provider profiles and routing are system-level configuration, not thread-local SDK setup.
- Provider adapters are drivers, not Agent Thread core.
- Model capability metadata controls tool visibility and streaming behavior.
- Provider request state is scoped to ModelTurnSession unless persisted by the kernel.
- Retry and repair behavior must emit events.
- Provider transforms must be conformance-tested.

Normative design: [Provider System](provider-system.md).

## 12. Context Projection

The Context Manager owns source context. Agent Thread owns the current projection request.

Projection rules:

- only scoped context can enter a turn
- every context entry carries provenance
- stale context is marked, not silently reused
- compaction emits explicit ContextCompaction items
- replacement history must be linked to the original history
- private reasoning is not shared between sibling Agent Threads by default

Child Agent Threads may receive:

```text
NoHistory
TaskOnly
LastNTurns
SelectedItems
FullHistory
```

`FullHistory` requires an explicit policy grant.

Child Agent Threads may trigger owner Memento Fragments through lifecycle events such as completion, but they do not receive those fragments as context. Memento projection is owner-scoped.

## 12.1 Communication Projection

Agent Threads do not receive all messages by default.

The Communication Kernel decides whether a message enters the receiver mailbox, triggers a receiver turn, waits for review, updates a blackboard projection, or is rejected.

Worker communication rights are assigned at creation time through a Communication Profile. For example, a worker may be allowed to report blockers to its Supervisor, post `Risk` entries to a goal-scoped blackboard channel, and be denied human messaging. Direct worker-to-worker messaging is not part of the v0.1 core route set.

## 13. Tool and Permission Pipeline

Tool calls execute through this pipeline:

```text
model proposal
  -> tool schema validation
  -> tool visibility check
  -> capability check
  -> risk classification
  -> deny/ask/allow rule evaluation
  -> lifecycle policy hooks
  -> optional risk classifier
  -> approval resolution
  -> sandbox selection
  -> driver execution
  -> output normalization
  -> evidence attachment
  -> audit event
  -> model-visible result projection
```

Decision precedence:

```text
hard deny
capability miss
role restriction
hook deny
explicit ask
classifier deny
allow
```

The classifier may reduce approval fatigue, but it cannot override hard deny, capability miss, role restriction, hook deny, or explicit ask.

## 14. Lifecycle Policy Hooks

Hooks are kernel extension points.

Initial hook events:

```text
ThreadStart
ThreadStop
TurnStart
TurnStop
PreModelCall
PostModelCall
PreToolUse
PostToolUse
PostToolBatch
PermissionRequest
PermissionDenied
ChildSpawn
ChildStop
WorktreeCreate
WorktreeRemove
PreCompact
PostCompact
PreFinalSubmit
```

Hook output may:

- deny an action
- request approval
- request stricter sandboxing
- add non-authoritative context
- attach evidence
- request review or verification
- emit audit metadata

Hook output may not:

- grant new capabilities
- mutate ATCB directly
- bypass Tool Broker
- alter durable history without a syscall
- hide audit events

## 15. Agent Control and Registry

Agent Control owns multi-thread operations.

Required responsibilities:

- create Agent Threads
- reserve spawn slots before creation
- release reservations on failure
- assign stable agent paths
- assign Supervisor levels (`S0`, `S1`, `S2`, ...)
- persist invocation edges for root Supervisors, Supervisor delegation, worker assignment, review request, and human escalation
- enforce max active threads
- enforce max depth
- persist parent-child edges
- route reports through Supervisor and shared state through scoped blackboards
- list live agents
- subscribe to child status
- unload resident idle children
- resume unloaded children from persisted state

Agent paths use slash-separated hierarchy:

```text
/
/explore-1
/coder/api
/review/security
```

Agent names are display metadata. Agent paths are routing identity.

## 16. Scheduling

Scheduling is cooperative in v0.1.

Yield boundaries:

- before model call
- after model call
- before tool call
- after tool result
- after tool batch
- before artifact commit
- after artifact commit
- before approval request
- after approval result
- before final submission
- after final decision
- before unload

Scheduler inputs:

- task readiness
- dependencies
- role priority
- resource locks
- tool/process limits
- budget remaining
- review requirements
- human approval state

Scheduler outputs:

- run
- wait
- block
- suspend
- unload
- quarantine
- terminate

The scheduler MUST NOT preempt an active model request mid-stream in v0.1.

The normative scheduling and arbitration rules live in [Scheduler and Resource Arbitration](scheduler-and-resource-arbitration.md). This section only defines where the Agent Thread must yield and what scheduler outcomes it must obey.

## 17. Isolation

Agent Thread isolation has five layers:

1. Context isolation: scoped projection only.
2. Permission isolation: capabilities and role restrictions.
3. Memory isolation: namespace-scoped read/write policies.
4. Workspace isolation: read-only, workspace-write, branch, worktree, container, or remote worker.
5. Process isolation: sandbox, cgroup, namespace, job object, container, or VM depending on platform.

The workspace and process layers are supplied by the [Execution Environment System](execution-environment-system.md), not by ad hoc thread-local setup.

Recommended defaults:

| Role | Workspace | Tool Risk Ceiling |
|---|---|---|
| SupervisorAgent | read-only | orchestration, approval, and final submission |
| WorkerAgent | read-only, workspace-write, isolated worktree, or temp outputs depending on assignment | scoped file CRUD and command execution |
| ReviewerAgent | read-only | review only |

Production distributions SHOULD make sandbox unavailability a hard failure for high-risk roles.

## 18. Recovery

Recovery is event-first.

Requirements:

- emit `ToolCallStarted` before external execution
- emit terminal tool event for completed, failed, timed out, denied, or cancelled calls
- checkpoint at every yield boundary
- materialize ATCB from events and latest checkpoint
- detect orphan running tools on restart
- reconcile workspace diff with artifact state
- never mark a final answer complete without verification state

Restart behavior:

```text
1. load last checkpoint
2. replay events after checkpoint
3. rebuild ATCB and turn state
4. reconcile running tools and background processes
5. mark unsafe incomplete operations as interrupted
6. resume only at a yield boundary
```

## 19. AgentThreadHandle API

The runtime SHOULD expose a handle similar to:

```rust
#[async_trait::async_trait]
pub trait AgentThreadHandle {
    async fn submit_op(&self, op: AgentOp) -> Result<AgentOpAck>;
    async fn try_start_turn(&self, op: AgentOp) -> Result<TurnStartAck>;
    async fn steer_turn(&self, op: AgentOp) -> Result<TurnSteerAck>;
    async fn interrupt_turn(&self, turn_id: TurnId) -> Result<()>;
    async fn inject_items(&self, items: Vec<AgentItem>) -> Result<()>;
    async fn status(&self) -> Result<ThreadStatusSnapshot>;
    async fn config_snapshot(&self) -> Result<ThreadConfigSnapshot>;
    async fn subscribe_events(&self, from: EventCursor) -> Result<EventStream>;
    async fn checkpoint(&self) -> Result<CheckpointId>;
    async fn shutdown(&self, reason: ShutdownReason) -> Result<()>;
}
```

This is a conceptual API. Final signatures belong in `agent-os-thread-protocol`.

## 20. Storage

v0.1 storage SHOULD use:

- SQLite WAL for local durable event log and projections
- local filesystem for artifact blobs
- deterministic JSON or Protobuf event encoding
- hash-addressed evidence blobs

PostgreSQL is a later storage driver, not kernel essence.

The thread store MUST support:

- append events
- read by thread id
- read by turn id
- replay from checkpoint
- list active threads
- list children by parent
- search by metadata
- archive and unarchive

## 21. Production Conformance Tests

Minimum conformance tests:

1. Invalid state transition is rejected.
2. `turn.start` is rejected while the thread is busy.
3. `turn.steer` rejects stale `expected_turn_id`.
4. Tool call without capability is denied and audited.
5. Denied tool call does not mutate artifacts.
6. Hook can deny a tool call but cannot grant missing capability.
7. Classifier allow cannot override hard deny.
8. WorkerAgent cannot be the sole reviewer or acceptor of its own artifact.
9. ReviewerAgent cannot write workspace files.
10. Child Agent Thread does not inherit parent permissions by default.
11. Spawn reservation is released when spawn fails.
12. Max active Agent Thread limit is enforced.
13. Resident idle child can unload and resume.
14. Context compaction emits replacement provenance.
15. Crash during tool execution recovers to interrupted or completed state.
16. Final submission without evidence map is rejected.
17. Supervisor can arm a Memento Fragment anchored to child completion.
18. Child cannot read or mutate parent Memento Fragments.
19. Triggered Memento Fragment is projected only to the owner thread.
20. Worker without Supervisor route cannot send Supervisor messages.
21. Worker without blackboard route cannot post blackboard messages.
22. Worker without human route cannot contact a human.

## 22. Implementation Iterations

### Iteration AT-0: Protocol Skeleton

Deliver:

- AgentOp schema
- AgentEvent schema
- AgentItem schema
- ThreadStatus and TurnStatus transition table
- Memento Fragment event types
- Communication Profile and message event types
- in-memory event stream
- mock Agent Thread without LLM

Gate:

- event replay reconstructs ATCB

### Iteration AT-1: Single Agent Thread Runtime

Deliver:

- AgentThreadHandle
- turn admission checks
- deterministic outer loop
- mock Provider System
- checkpoint manager

Gate:

- thread can start, block, resume, complete, and recover from checkpoint

### Iteration AT-2: Provider System

Deliver:

- provider profile resolution
- model alias resolution
- provider-neutral streaming interface
- ModelTurnSession
- model capability catalog
- routing and fallback policy
- provider transform contract
- token usage accounting

Gate:

- no provider turn state leaks across turns
- thread runtime does not call provider SDKs directly

### Iteration AT-3: Tool Syscall Loop

Deliver:

- ToolCall lifecycle
- Tool Broker integration
- permission decision events
- sandbox selection placeholder
- tool result projection

Gate:

- denied, failed, cancelled, and completed tool calls are all replayable

### Iteration AT-4: Context and Compaction

Deliver:

- typed context projection
- context freshness metadata
- ContextCompaction item
- selected-history child spawn modes

Gate:

- model-visible prompt can be regenerated from typed state

### Iteration AT-5: Agent Control

Deliver:

- AgentRegistry
- spawn reservations
- parent-child edges
- inter-agent messages
- Communication Profile enforcement
- blackboard channel posting
- human message routing
- Memento Fragment child-completion anchors
- max active thread limits
- resident unload/resume

Gate:

- child Agent Thread lifecycle is durable and recoverable

### Iteration AT-6: Isolation and Hooks

Deliver:

- lifecycle hook bus
- read-only and workspace-write profiles
- worktree allocation driver
- sandbox hard-fail policy

Gate:

- hooks cannot grant missing permissions
- role isolation is enforced by conformance tests

### Iteration AT-7: Evidence and Final Contract

Deliver:

- evidence attachment in turn loop
- artifact links
- review and verification request events
- final submission gate

Gate:

- SupervisorAgent cannot complete high-impact work without evidence-backed final contract

## 23. Out-of-Scope for v0.1

The following are not required for the first Agent Thread implementation:

- distributed scheduling
- remote worker leases
- PostgreSQL production deployment
- marketplace packages
- graphical console
- third-party distribution certification
- preemptive interruption of in-flight model generation

They must not distort the core Agent Thread ABI.
