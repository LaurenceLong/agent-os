# Agent Thread Runtime

Status: normative foundation

Last updated: 2026-06-26

Canonical v0.1 field schemas, event names, and `ThreadStatus` values are defined in [Agent Thread Core Module](agent-thread-core-module.md). This document defines the conceptual runtime contract and built-in role model.

## 1. Purpose

The Agent Thread Runtime executes kernel-managed Agent Threads.

An Agent Thread is the dedicated execution unit of Agent-OS. It is analogous to a thread in a traditional operating system, but its state is semantic and governance-oriented rather than CPU-register-oriented.

An Agent Thread MUST NOT be defined as:

- a raw prompt
- a chat completion loop
- a LangGraph node
- an AutoGen agent
- a CrewAI role
- an OpenHands agent
- a generic function-calling wrapper

Those systems can integrate only through distribution packages or external tools
that use Agent-OS kernel contracts directly. The Agent Thread Runtime is part of
Agent-OS core infrastructure.

## 2. Agent Control Block

Every Agent Thread MUST have an Agent Control Block.

Conceptual ACB fields:

The v0.1 implementation uses the Agent Thread Control Block (ATCB) schema in [Agent Thread Core Module](agent-thread-core-module.md#6-agent-thread-control-block). The conceptual fields below explain the intent behind the control block; they are not the canonical field list when names differ.

```yaml
agent_id: string
parent_id: string | null
session_id: string
task_id: string
role: string
owner: string
state: AgentState
priority: integer
goal:
  global_goal: string
  local_goal: string
  current_subgoal: string | null
  success_criteria: string[]
  failure_criteria: string[]
  deadline: string | null
context:
  context_id: string | null
  loaded_refs: string[]
  summary_ref: string | null
  freshness: string
  pollution_score: number
memory:
  readable_namespaces: string[]
  writable_namespaces: string[]
tools:
  allowed_tools: string[]
  denied_tools: string[]
permissions:
  capabilities: string[]
  risk_ceiling: integer
  approval_required_above: integer
budget:
  token_budget: integer | null
  tool_call_budget: integer | null
  wall_time_budget_ms: integer | null
  cost_budget: number | null
dependencies:
  depends_on: string[]
  blocks: string[]
risk_level: integer
evidence:
  required: string[]
  submitted: string[]
progress:
  checkpoint_id: string | null
  progress_score: number
  last_meaningful_event_id: string | null
artifacts:
  owned: string[]
  consumed: string[]
audit:
  created_at: string
  updated_at: string
  created_by: string
  termination_reason: string | null
```

The ACB is kernel-owned. Agent Threads may read their visible ACB fields, but they MUST NOT mutate the ACB directly.

## 3. Lifecycle

Conceptual Agent Thread states:

The v0.1 `ThreadStatus` enum is defined in [Agent Thread Core Module](agent-thread-core-module.md#71-threadstatus). The states below describe the broader lifecycle vocabulary used by the runtime.

```text
Created
Ready
Planning
Thinking
CallingModel
CallingTool
WaitingTool
WaitingHuman
Blocked
Reviewing
Revising
Suspended
Completed
Failed
Quarantined
Terminated
```

All state transitions MUST be validated by the kernel.

Example transition rules:

- `Created -> Ready` requires a valid ACB.
- `Ready -> Thinking` requires dependency readiness.
- `Thinking -> CallingTool` requires a proposed syscall.
- `CallingTool -> WaitingTool` requires Tool Broker acceptance.
- `WaitingTool -> Thinking` requires tool result or failure event.
- `Thinking -> Completed` requires required evidence or explicit waiver.
- `Blocked -> Ready` requires blocker resolution.
- `Any -> Quarantined` is allowed for policy violation or suspicious behavior.
- `Completed -> Terminated` is allowed after final audit closure.

## 4. Execution Loop

Agent Thread execution MUST follow a kernel-controlled outer loop:

```text
1. receive assignment
2. read visible ACB
3. request scoped context
4. think with bounded context
5. propose action
6. call kernel syscall
7. wait for policy/tool/evidence result
8. update progress through syscall
9. yield checkpoint
10. continue, block, revise, complete, or fail
```

The LLM is used inside step 4 and MAY assist in step 5. It MUST NOT bypass steps 6-9.

## 5. Yield and Checkpoint Semantics

Agent Threads MUST yield at these boundaries:

- before model call
- after model call
- before tool call
- after tool result
- before artifact commit
- after artifact commit
- before approval request
- after approval result
- before final submission
- when blocked
- when budget threshold is reached

Each yield SHOULD create or update a checkpoint.

The kernel may suspend, resume, compact, replan, or terminate an Agent Thread only at yield boundaries in v0.1.

## 6. Core Roles and Supervisor Levels

Agent-OS v0.1 keeps the core role set small:

```text
SupervisorAgent
WorkerAgent
ReviewerAgent
```

Distribution prompts, examples, and policy packs MAY define workflow step labels. Those labels map onto the core roles; they are not kernel-required roles.

Supervisors are hierarchical. The top Supervisor for a goal is `S0`. If `S0` delegates a sub-organization to another Supervisor, that Supervisor is `S1`. Further delegation increments the level (`S2`, `S3`, ...).

The kernel MUST record every delegation, worker assignment, review request, and human escalation as an invocation edge. The invocation graph is the source of truth for responsibility tracing, cancellation, replay, audit, and permission boundary analysis.

### 6.1 SupervisorAgent

Responsibilities:

- own top-level goal
- define acceptance criteria
- create task DAG
- assign Agent Threads
- decide whether to use division of labor, parallelism, checks and balances, shared blackboard, or tighter permission boundaries
- delegate to lower-level Supervisors when a sub-organization is needed
- resolve conflicts
- decide final acceptance
- request human approval when required

Restrictions:

- SHOULD NOT directly modify workspace files
- MUST NOT fabricate evidence
- MUST NOT accept final output without required evidence
- MUST NOT hide or rewrite invocation edges

### 6.2 WorkerAgent

Responsibilities:

- perform assigned work
- inspect scoped files, logs, docs, issues, and system state
- modify artifacts only when its permission and sandbox profiles allow it
- run tests or commands when assigned
- attach source, diff, and command evidence

Restrictions:

- MUST stay inside its assignment and capability scope
- MUST NOT be the sole reviewer or acceptor of its own artifact
- MUST NOT widen its own role, permission, sandbox, communication, or Supervisor level

### 6.3 ReviewerAgent

Responsibilities:

- inspect artifacts independently
- review correctness, edge cases, regressions, security, maintainability, and evidence quality
- submit review findings against exact artifact versions

Restrictions:

- MUST be read-only toward reviewed artifacts
- MUST NOT rewrite the artifact under review
- SHOULD NOT share the producer's full private reasoning context

### 6.4 Testing and Verification Are Responsibilities

Testing is a WorkerAgent responsibility. A worker that runs tests or reproduces failures MUST attach command evidence.

Verification is primarily a kernel gate over final submissions. The kernel checks:

- evidence exists
- evidence is current
- evidence matches artifact versions
- required review is independent
- test logs match stated commands
- final claims do not exceed evidence

A distribution MAY create verification workflow steps, but the kernel MUST NOT depend on a dedicated verification role.

## 7. Compatibility Guests

Open-source agents MAY be executed as guest runtimes.

Guest runtimes MUST be wrapped so that:

- all tool calls pass through Tool Broker
- all state changes pass through kernel syscalls
- all artifacts are committed through Artifact Store
- all evidence is registered in Evidence Store
- all permissions are enforced by Permission Kernel

If a guest runtime cannot obey these constraints, it is not compatible with Agent-OS.
