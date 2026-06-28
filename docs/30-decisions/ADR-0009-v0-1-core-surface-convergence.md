# ADR-0009: v0.1 core surface is minimal tools, supervised collaboration, and scoped blackboards

Status: accepted

Date: 2026-06-26

## Context

Early Agent-OS design documents mixed three different surfaces:

- model-visible function tools
- Agent-OS kernel syscalls and internal tools
- distribution-specific workflow roles and pipelines

That made the system look like a multi-agent framework with many named roles and tools, rather than a small operating-system control plane. The v0.1 surface needs to be smaller and more stable.

## Decision

Agent-OS v0.1 will converge on a typed model-visible tool taxonomy.

Host OS tools are exactly:

```text
read_file
write_file
replace_text
delete_file
run_command
```

These are substrate tools. They expose the minimum file and process capabilities that a hosted operating system must provide to an agent. They do not include Agent-OS control-plane actions.

Agent-OS control-plane tools are separate:

```text
work state:
  set_objective
  update_checklist
  record_evidence

communication:
  report_supervisor
  post_blackboard
  ask_human

agent supervision:
  agent_control

session lifecycle:
  submit_final
```

`agent_control` is one CLI-like model tool with an `action` field, not a family of separate model tools. Supported actions include:

```text
start
status
output
set_hook
send
resume
stop
set_timeout
export_trace
kill
delete_session
purge_state
```

`start` MAY set multiple control-plane properties atomically, including assignment, workdir, profile, model hint, timeout, output policy, and initial hooks. `kill`, `delete_session`, and `purge_state` are privileged actions hidden from normal WorkerAgent views and require Supervisor policy plus explicit permission.

`submit_final` is a lifecycle action exposed as a model function, not a filesystem tool.

Everything below the model-visible tool taxonomy is an Agent-OS syscall, internal driver, policy hook, or distribution feature. Examples include approval, scheduling, memory, provider routing, PTY management, log normalization, process groups, and durable storage.

The core role set is:

```text
SupervisorAgent
WorkerAgent
ReviewerAgent
```

Supervisors are hierarchical. The top-level Supervisor for a goal is `S0`. A Supervisor created by delegation from `S0` is `S1`; further Supervisor delegation increments the level (`S2`, `S3`, and so on). The level is control-plane state, not a prompt convention.

Every Supervisor or worker creation caused by delegation MUST create a durable invocation edge. The edge records at least:

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
relationship: supervisor_delegation | worker_assignment | review_request | human_escalation
assignment: string
capability_snapshot_id: string | null
profile_snapshot_id: string
created_at: string
```

The invocation graph is used for replay, audit, cancellation, responsibility tracing, and permission boundary checks.

Agent supervision MUST NOT be a fire-and-forget spawn. A started agent has durable state, captured output, a session id when the provider supports one, timeout policy, liveness state, and a control channel for follow-up instructions.

`wait_agent` is not a core v0.1 tool. Progress reporting is managed by hooks instead of blocking waits or done markers. `agent_control(action: "set_hook")` registers a kernel-scheduled hook that periodically injects a bounded prompt into the child agent and routes the response to Supervisor:

```yaml
agent_id: string
hook_type: progress_report
interval_seconds: integer
prompt: string
response_route: supervisor
max_response_chars: integer
stop_when: terminal | cancelled
on_missed_reports: report | escalate | stop
```

The default progress report prompt SHOULD ask for one short status sentence covering progress, blockers, and next action. The hook result is a communication event, not a final answer and not blackboard truth.

Testing is a Worker task that runs commands and attaches command evidence. Verification is primarily a kernel final-submission gate that checks evidence coverage, stale artifact references, unsupported claims, and independence requirements.

The kernel MUST NOT hard-code an official software-engineering pipeline. Distributions should provide workflow prompts, examples, and policy packs. The Supervisor decides at runtime whether to divide work, run parallel workers, request review, use the blackboard, or escalate to a human.

Agent collaboration is defined by five mechanisms:

```text
1. division of labor
2. parallelism
3. checks and balances
4. shared blackboard
5. permission and responsibility boundaries
```

Agent-to-agent coordination goes through Supervisor. Parent-child task structure is represented in the task and invocation graph, but direct worker-to-worker routes are not separate default communication capabilities.

The core communication routes are:

```text
supervisor
blackboard
human
```

The blackboard MUST be scoped and channel-based. Minimum scopes:

```text
global
goal
task
```

Minimum channels:

```text
facts
risks
blockers
artifacts
evidence
questions
decisions
```

## Consequences

Positive:

- the model-facing surface is small enough to reason about
- Host OS substrate tools remain minimal without hiding Agent-OS control-plane tools
- Agent-OS stays an operating-system control plane instead of a workflow template
- distributions can still define richer roles without changing kernel semantics
- communication is auditable and avoids unbounded chat meshes
- child agents remain supervised through status, output, hook, timeout, resume, and stop controls
- verification becomes enforceable below prompts

Negative:

- existing workflow examples must be rewritten as distribution examples, not kernel requirements
- exploratory convenience behavior must move behind `run_command`, context projection, or internal Agent-OS services
- implementations need a `delete_file` tool to complete the converged filesystem CRUD set
- agent supervision requires more state than a simple spawn tool

## Supersedes

This ADR narrows the implications of ADR-0003 and ADR-0006:

- ADR-0003 still stands that Agent Thread Runtime is proprietary core infrastructure, but the runtime core role set is narrowed to Supervisor, Worker, and Reviewer.
- ADR-0006 still stands that communication is capability-scoped, but v0.1 direct communication is Supervisor-routed.
