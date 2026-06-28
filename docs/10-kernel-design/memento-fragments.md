# Memento Fragments

Status: normative

Last updated: 2026-06-25

## 1. Purpose

Memento Fragment is the Agent-OS primitive for an Agent Thread to leave an immutable reminder for its future self.

The metaphor is the film *Memento*: the agent cannot rely on a continuous, perfectly remembered internal stream, so it uses external memory shards to resume correctly after delegation, suspension, compaction, approval waits, long-running tools, or child-agent completion.

This is not a parent-child note-passing mechanism. A Memento Fragment is owned by the thread that writes it. A child Agent Thread may trigger the reminder by completing work, but it MUST NOT read, edit, delete, or reinterpret the parent's Memento Fragment.

## 2. Definition

A Memento Fragment is a small, immutable, owner-scoped self-reminder anchored to a future event.

Example:

```text
When the auth exploration worker completes, read its evidence refs first, then decide whether to assign the edit worker or ask the user for scope.
```

Another example:

```text
After the long test command returns, do not summarize immediately. First check whether failures are deterministic or environment-only.
```

## 3. Non-Goals

Memento Fragments are not:

- instructions to a child Agent Thread
- child-visible spawn prompts
- long-term memory
- evidence
- blackboard facts
- task acceptance criteria
- hidden mutable scratchpads
- permission grants

Child-facing instructions belong in the child assignment payload. Durable knowledge belongs in MemoryRecord. Shared state belongs in Typed Blackboard. Proof belongs in Evidence.

## 4. Core Invariants

1. A Memento Fragment is written by exactly one owner Agent Thread.
2. The committed content is immutable.
3. A child Agent Thread cannot mutate a parent's Memento Fragment.
4. A child Agent Thread cannot read a parent's Memento Fragment. If the owner wants to tell the child something, it must create a separate child-visible assignment or context object.
5. A child Agent Thread can only produce events that satisfy or trigger a Memento Fragment's anchor.
6. The owner cannot edit a committed Memento Fragment; it can only supersede it with a new fragment.
7. A Memento Fragment cannot grant capabilities or override policy.
8. Projection is to the owner thread at resume or trigger time, not to arbitrary agents.

## 5. Core Use Cases

### 5.1 Delegation Callback

Before spawning a child Agent Thread, the parent creates a Memento Fragment for itself.

```text
Spawn a WorkerAgent for the storage layer. When it completes, compare its findings against ADR-0002 before deciding PostgreSQL placement.
```

The child receives its own assignment, not the Memento Fragment. When the child completes, the kernel triggers the parent's Memento Fragment.

### 5.2 Resume Reminder

Before yielding, waiting for approval, or entering a long tool call, an Agent Thread records what it must do next.

```text
After approval returns, if denied, produce a lower-risk plan instead of retrying the same command.
```

### 5.3 Compaction Anchor

Before compaction, the Agent Thread records a short self-reminder that survives context replacement.

```text
After compaction, preserve the distinction between source study and normative design.
```

### 5.4 Review Continuation

Before requesting a review, the producer records how it should react when the review comes back.

```text
When ReviewerAgent returns P1 findings, revise artifact first; do not pass the final verification gate until review findings are resolved.
```

## 6. Data Shape

Canonical logical fields are also listed in [Kernel Data Model](kernel-data-model.md#23-mementofragment). The shape below is the same contract from the Memento subsystem's point of view.

```yaml
memento_id: string
owner_thread_id: string
owner_agent_id: string
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

## 7. Lifecycle

```text
Draft
  -> Armed
  -> Triggered
  -> Projected
  -> Consumed
```

Alternate terminal states:

```text
Superseded
Expired
Invalidated
```

Rules:

- `Draft -> Armed` commits the content hash.
- After `Armed`, content is immutable.
- `Triggered` means the anchor condition happened.
- `Projected` means the owner saw the reminder in its context.
- `Consumed` means the owner acknowledged or acted on it.
- `Superseded` requires a new Memento Fragment id.
- `Invalidated` does not delete the old fragment.

## 8. Kernel Operations

### 8.1 `memento.create`

Create a draft Memento Fragment.

### 8.2 `memento.arm`

Commit and arm the fragment. This computes content hash and freezes the content.

### 8.3 `memento.trigger`

Kernel operation emitted when the anchor condition is satisfied.

Child Agent Threads do not call this directly; their lifecycle events may cause the kernel to trigger the memento.

### 8.4 `memento.project`

Project triggered Memento Fragments into the owner Agent Thread's next turn or resume context.

### 8.5 `memento.consume`

Owner acknowledges that the reminder has been handled.

### 8.6 `memento.supersede`

Owner creates a new Memento Fragment that replaces an older one.

### 8.7 `memento.invalidate`

Kernel or owner invalidates an unsafe, stale, or incorrect fragment.

## 9. Spawn Integration

`agent.spawn_child` SHOULD allow a parent to arm one or more Memento Fragments bound to the child completion event.

Example:

```yaml
spawn_child:
  role: WorkerAgent
  assignment: "Inspect state-storage design and report missing replay invariants."
  owner_mementos:
    - title: "After explorer returns"
      body: "Compare returned gaps against agent-thread-core-module.md and update conformance tests before continuing."
      anchor:
        anchor_type: child_thread_completed
        anchor_ref: "$child_thread_id"
```

The `assignment` is child-visible. `owner_mementos` are not child-visible.

## 10. Projection Rules

Memento Fragments are projected only to the owner thread unless a future ADR explicitly expands the model.

Projection ordering:

1. critical triggered mementos
2. high priority triggered mementos
3. mementos anchored to active child completions
4. resume reminders
5. normal priority reminders

Projection MUST include:

- memento id
- title and body
- anchor that triggered it
- related child/tool/artifact/evidence refs
- supersession or invalidation status if applicable

## 11. Relationship to Other Objects

| Object | Difference |
|---|---|
| Child assignment | Child-visible work request; Memento Fragment is owner-visible self-reminder |
| ContextSnapshot | Snapshot of loaded context; Memento Fragment is future continuation intent |
| MemoryRecord | Durable memory; Memento Fragment is task-scoped and short-lived |
| BlackboardEntry | Shared state; Memento Fragment is owner-scoped |
| Evidence | Proof; Memento Fragment is not proof |
| Artifact | Deliverable; Memento Fragment is not a deliverable |
| AuditEvent | Records action; Memento Fragment records future intent |

## 12. Safety Rules

1. Child Agent Threads cannot read parent Memento Fragments.
2. Child Agent Threads cannot mutate parent Memento Fragments under any circumstance.
3. Tool calls cannot mutate Memento Fragments.
4. A Memento Fragment cannot become MemoryRecord automatically.
5. A Memento Fragment cannot be used as evidence unless separately verified and promoted.
6. A Memento Fragment cannot satisfy task completion criteria by itself.
7. Triggered Memento Fragments must be clearly labeled as reminders, not facts.
8. Expired Memento Fragments must not be projected in normal context.

## 13. Conformance Tests

Minimum tests:

1. Parent can create and arm a child-completion Memento Fragment.
2. Child cannot read the parent's Memento Fragment.
3. Child cannot mutate the parent's Memento Fragment.
4. Child completion triggers the parent's Memento Fragment.
5. Triggered Memento Fragment is projected to the parent on resume.
6. Parent cannot edit armed content; it must supersede with a new fragment.
7. Superseded fragment remains auditable.
8. Memento Fragment cannot grant a missing capability.
9. Memento Fragment cannot become evidence without explicit promotion.
10. Event replay reconstructs Memento Fragment lifecycle.
