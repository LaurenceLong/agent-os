# State, Storage, and Replay

Status: normative

Last updated: 2026-06-25

## 1. Principle

Agent-OS state MUST be durable, typed, and replayable.

The source of truth is not process memory and not chat history. The source of truth is the combination of:

- append-only events
- current-state projections
- role, permission, and sandbox profiles
- execution environments and their leases
- scheduler policies, resource leases, and budget ledgers
- provider profiles and routing policies
- immutable artifact blobs
- immutable evidence blobs
- explicit approval records
- communication messages and delivery events
- immutable Memento Fragments
- versioned memory records

## 2. Store Abstractions

The kernel MUST depend on storage traits, not a specific database product.

Initial traits:

```text
EventStore
ProjectionStore
ArtifactBlobStore
EvidenceBlobStore
LockStore
LeaseStore
EnvironmentStore
ProfileStore
SchedulerStore
MessageStore
MementoStore
MemoryStore
ProviderStore
AuditStore
```

SQLite and PostgreSQL are drivers behind these traits.

## 3. Local and Production Drivers

### 3.1 SQLite Driver

SQLite is the default local driver.

Use cases:

- single-user local development
- deterministic tests
- simple demos
- offline replay
- compact bug reports

SQLite MUST support the same logical schema as the production driver where practical.

### 3.2 PostgreSQL Driver

PostgreSQL is the official production control-plane driver.

Use cases:

- multiple workers
- long-running projects
- audit queries
- operational dashboards
- row-level namespace isolation
- transactional task and event updates
- production backup and restore

PostgreSQL MUST NOT be treated as part of kernel identity. It is a production storage driver.

### 3.3 Object Storage Driver

Large blobs SHOULD be stored outside the control-plane database.

Examples:

- command logs
- screenshots
- videos
- benchmark traces
- generated archives
- large diffs

The control-plane store keeps metadata and content hashes.

## 4. Event Sourcing Model

Every important state transition MUST emit an event.

Examples:

- `GoalRegistered`
- `TaskSpawned`
- `ThreadConfigured`
- `ThreadStatusChanged`
- `TurnStarted`
- `TurnCompleted`
- `TurnFailed`
- `TurnBlocked`
- `RoleBindingResolved`
- `ContextLoaded`
- `EnvironmentProvisioned`
- `EnvironmentLeaseGranted`
- `EnvironmentLeaseReleased`
- `ResourceLeaseGranted`
- `ResourceLeaseReleased`
- `BudgetReserved`
- `BudgetDebited`
- `BudgetExhausted`
- `ProviderProfileResolved`
- `ProviderStreamSessionOpened`
- `ProviderRetry`
- `ProviderStreamFailed`
- `ProviderUsageRecorded`
- `CommunicationMessageSent`
- `CommunicationMessageDelivered`
- `CommunicationMessageRejected`
- `BlackboardPostSubmitted`
- `BlackboardPostPublished`
- `HumanMessageRequested`
- `HumanMessageDelivered`
- `MementoFragmentArmed`
- `MementoFragmentTriggered`
- `MementoFragmentProjected`
- `MementoFragmentConsumed`
- `MementoFragmentSuperseded`
- `MementoFragmentInvalidated`
- `ToolCallProposed`
- `ToolCallApprovalRequested`
- `ToolCallApprovalResolved`
- `ToolCallStarted`
- `ToolCallProgressed`
- `ToolCallCompleted`
- `ToolCallFailed`
- `ArtifactCommitted`
- `EvidenceAttached`
- `ReviewRequested`
- `ReviewSubmitted`
- `VerificationSubmitted`
- `ApprovalRequested`
- `ApprovalRecorded`
- `ConflictRaised`
- `ConflictResolved`
- `FinalSubmitted`

Agent Thread events MUST use the names defined in [Agent Thread Core Module](agent-thread-core-module.md#82-agentevent-envelope). Storage drivers may expose aggregate-specific aliases only when the ABI defines an explicit mapping.

Events are immutable.

Projections can be rebuilt from events.

## 5. Replay

Replay modes:

```text
audit_replay
  Reconstructs what happened for inspection.

state_replay
  Rebuilds current projections from event history.

simulation_replay
  Replays a task with model and tool calls mocked.

debug_replay
  Replays until a selected event and exposes state.
```

The v0.1 kernel MUST support `state_replay` and `audit_replay`.

`simulation_replay` SHOULD be implemented before distributed workers.

## 6. Idempotency

All syscalls that can create side effects MUST carry an idempotency key.

The kernel MUST detect repeated syscalls and return the original result where safe.

Tool drivers MUST declare idempotency support:

```text
none
kernel_deduplicated
tool_native
manual_compensation_required
```

Non-idempotent high-risk tool calls SHOULD require approval.

`ToolCallProgressed` records non-terminal progress for a started invocation.
The canonical use is the 15 second foreground wait cap: the invocation becomes
`Running`, model-visible output includes `tool_call_id`, and the background
worker later emits `ToolCallCompleted` or `ToolCallFailed`. Replay MUST preserve
`Running` as non-terminal until a terminal event for the same call id appears.

## 7. Locks and Leases

Agent-OS MUST prevent conflicting mutation.

Initial lock resources:

- file path
- artifact id
- environment id
- provider slot
- memory namespace
- task id
- deployment target
- external account
- human attention

Locks MUST include:

- owner agent id
- task id
- lease expiration
- reason
- risk level

Expired locks MAY be reclaimed by the kernel after audit.

## 8. Migrations

Schema migrations MUST be versioned.

Rules:

- migrations MUST be reversible in local development where possible
- production migrations MUST include backup guidance
- ABI-breaking migrations MUST have ADRs
- event schema changes MUST preserve deterministic replay for the current schema
  and current event model

## 9. Backup and Export

Production distributions SHOULD support:

- full control-plane backup
- artifact metadata export
- evidence metadata export
- audit log export
- selected task bundle export
- replay bundle export

The task bundle export is critical for debugging and third-party conformance tests.
