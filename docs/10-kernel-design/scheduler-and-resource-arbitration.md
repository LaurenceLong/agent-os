# Scheduler and Resource Arbitration

Status: normative

Last updated: 2026-06-25

## 1. Purpose

Agent-OS MUST schedule turns and arbitrate scarce resources at the kernel level.

An Agent Thread may request work, spawn children, or ask for tools, but it does not decide on its own when it runs, which exclusive resources it may hold, or whether enough budget remains to continue.

## 2. Why This Is Not Thread-Local

The following production concerns cannot live inside thread loops:

- fair scheduling across many tasks and roles
- file and workspace mutation conflicts
- provider concurrency limits
- execution environment capacity
- human attention as a schedulable resource
- review and verification lane priority
- budget reservation before expensive work
- starvation and priority inversion

These are organization-level control-plane responsibilities.

## 3. Responsibilities

The Scheduler and Resource Arbitration subsystem owns:

- ready queues
- cooperative turn dispatch
- resource lease grants and releases
- budget reservation and consumption tracking
- blocked and deferred state classification
- retry and backoff policy
- starvation prevention
- loop and deadlock detection
- suspension and quarantine decisions

## 4. Scheduling Model

Scheduling is cooperative in v0.1.

Dispatch boundaries include:

- before model call
- after model call
- before tool call
- after tool result
- before artifact commit
- before approval request
- before final submission
- after task transition

The scheduler MUST NOT preempt an active model stream mid-generation in v0.1.

## 5. Resource Model

The scheduler arbitrates more than CPU-like turns.

Initial resource classes:

- file mutation lanes
- workspace roots
- execution environments
- provider slots
- blackboard channels
- artifact review lanes
- deployment targets
- human attention

Human attention is a first-class scarce resource. A worker may have permission to message a human, but delivery and interruption timing are still subject to scheduler policy.

## 6. Logical Schemas

### 6.1 SchedulerPolicy

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

### 6.2 ResourceLease

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

### 6.3 BudgetLedger

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

## 7. Budget Semantics

Budget is not only a reporting counter. It is an admission-control mechanism.

The scheduler SHOULD reserve budget before:

- high-token model calls
- expensive test plans
- fan-out child creation
- human interrupts
- long-running external operations

Budget exhaustion MAY suspend or deny future scheduling, but it MUST NOT rewrite historical usage or hide already-consumed work.

## 8. Arbitration Rules

The scheduler MUST consider:

- task readiness from the DAG
- role family and scheduling class
- resource lease conflicts
- provider availability
- environment availability
- review and verification critical path
- human approval state
- remaining budget
- blocked reason taxonomy

The scheduler MAY downgrade or defer work that is technically runnable but low-value under current budget or attention pressure.

## 9. Priority Inversion and Starvation

The system MUST have explicit policy for:

- urgent review or verification work blocked by lower-priority producers
- low-priority tasks that never receive time
- parent threads waiting forever on noisy children
- provider slot starvation under mixed model classes

v0.1 MAY solve this with simple priority boosts and fairness windows before more advanced heuristics are introduced.

## 10. Relationship to Other Subsystems

- Task DAG Manager supplies readiness.
- Role and Profile System supplies scheduling class defaults.
- Execution Environment System supplies environment capacity and leaseable instances.
- Provider System supplies provider slot pressure and usage signals.
- Communication Kernel routes messages, but the scheduler decides whether they trigger receiver turns now or later.
- Approval and human messaging flows consume human attention budget.

## 11. First Implementation Target

The first production implementation SHOULD support:

- cooperative ready queue scheduling
- file and workspace conflict arbitration
- environment lease arbitration
- provider slot admission control
- budget ledgers for tokens, tool calls, wall time, cost, and human interrupts
- starvation detection with simple boosts
