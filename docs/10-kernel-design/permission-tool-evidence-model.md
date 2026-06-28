# Permission, Tool, and Evidence Model

Status: normative

Last updated: 2026-06-26

## 1. Purpose

This document defines the shared model for permissions, tool execution, evidence, review, verification, and final output.

Agent-OS is only production-grade if it can answer:

```text
Who did what?
For which goal?
With which permission?
Against which resource?
What evidence supports the claim?
Who reviewed it?
What remains unverified?
```

## 2. Risk Levels

Initial risk levels:

| Level | Name | Description | Default Handling |
|---|---|---|---|
| 0 | ThinkOnly | no external read or side effect | allow |
| 1 | ReadOnly | read local state, docs, logs | allow if scoped |
| 2 | DraftMutation | modify draft or temporary artifact | allow if scoped |
| 3 | WorkspaceMutation | modify source or project files | capability required |
| 4 | CommandExecution | execute local commands | allowlist or approval |
| 5 | NetworkOrExternalAPI | network, SaaS, external APIs | domain-scoped capability |
| 6 | IrreversibleOrProduction | deploy, delete, email send, production data | human approval required |

Risk level is evaluated per syscall, not only per agent role.

## 3. Effective Authorization

Authorization is an intersection, not a single flag.

The kernel SHOULD evaluate at least:

```text
Role Profile
  intersect Permission Profile
  intersect Capability Token
  intersect Sandbox Profile
  intersect Environment Lease and Resource Lease state
  intersect Approval scope
  intersect Budget state
```

A request is allowed only if every relevant control-plane check passes.

This is why role labels, prompt instructions, or tool visibility alone never count as authority.

## 4. Capability Tokens

Capabilities are scoped permissions.

Capability fields:

```yaml
capability_id: string
agent_id: string
task_id: string
role: string
syscalls: string[]
resources: string[]
risk_ceiling: integer
expires_at: string | null
approval_id: string | null
rate_limit: string | null
audit_required: boolean
```

Capabilities SHOULD be short-lived for high-risk actions.

Communication routes are capability-controlled resources. Examples include `comm:supervisor`, `blackboard:channel:<id>`, `blackboard:global`, and `human:message`.

## 5. Approval and Human Attention

Approval and human messaging are related but distinct.

Rules:

- `approval.request` asks for a decision record with bounded scope
- `human.message` asks to spend human attention on communication
- an approval does not authorize unrelated future work
- a human message does not by itself grant approval
- approval reuse outside recorded scope MUST be rejected
- human-facing interruption MAY be deferred or denied because of attention budget even when route capability exists

Minimum approval scope:

```yaml
approval_scope:
  syscall_types: string[]
  resource_scopes: object[]
  risk_ceiling: integer
  goal_id: string
  task_id: string | null
  expires_at: string | null
```

Human attention is modeled as scarce control-plane state through budget ledgers and resource leases, not as free-form chat availability.

## 6. Tool Broker

All tool calls MUST go through Tool Broker.

The v0.1 model-visible tool surface is typed by domain.

Host OS substrate tools are exactly:

```text
read_file
write_file
replace_text
delete_file
run_command
```

Agent-OS control-plane tools are:

```text
set_objective
update_checklist
record_evidence
report_supervisor
post_blackboard
ask_human
agent_control
submit_final
```

`agent_control` is a single CLI-like model tool with an `action` field. Normal actions are:

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
```

Privileged `agent_control` actions are:

```text
kill
delete_session
purge_state
```

`submit_final` is a model-visible lifecycle action, not a filesystem driver. Privileged `agent_control` actions require explicit Supervisor permission and MUST NOT appear in normal WorkerAgent tool views.

`wait_agent` is not a core tool. Agent progress is handled through status reads, output windows, lifecycle events, and `agent_control(action=set_hook)`.

Tool Broker responsibilities:

- normalize tool metadata
- validate input schema
- check capability
- check effective role and permission binding
- check resource scope
- check risk level
- check environment attachment and writable lease state where applicable
- check budget admission
- request approval when required
- execute through driver
- capture stdout, stderr, return code, and structured output
- redact secrets
- attach evidence
- record audit event

Agent Threads MUST NOT call tools directly.

## 6.1 Agent Supervision Tool Semantics

`agent_control(action=start)` MUST create durable agent state and an invocation edge before launching provider work. The `start` action MAY set multiple control-plane properties atomically, including timeout policy, output policy, and initial hooks.

The `agent_control` input shape is action-specific, but all actions share this envelope:

```yaml
action: start | status | output | set_hook | send | resume | stop | set_timeout | export_trace | kill | delete_session | purge_state
agent_id: string | null
thread_id: string | null
idempotency_key: string
payload: object
```

A started agent record includes:

```yaml
agent_id: string
thread_id: string
invocation_id: string
supervisor_level: integer | null
status: string
workdir: string
timeout_seconds: integer
session_id: string | null
output_handle: string
```

Example `start` payload:

```yaml
role_profile_id: role_worker
assignment: "Inspect storage replay gaps and report findings."
workdir: "/repo"
model_hint: "coding-primary"
timeout_seconds: 1200
output_policy:
  cursor_mode: new
  max_chars: 6000
hooks:
  - hook_type: progress_report
    interval_seconds: 120
    prompt: "Report one sentence: progress, blocker, next action."
    response_route: supervisor
    max_response_chars: 200
    stop_when: terminal
```

`agent_control(action=set_hook)` registers a kernel-scheduled control-plane hook:

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

The progress hook periodically injects the prompt into the child agent and routes the bounded response to Supervisor. Hook responses are status events, not evidence and not accepted blackboard facts unless separately recorded.

`agent_control(action=send)` is a live follow-up on an active child session. `agent_control(action=resume)` starts a new provider process against a persisted session id. `agent_control(action=stop)` requests graceful stop and preserves resumability. `agent_control(action=kill)` is privileged and may break resumability.

## 7. Tool Driver Classes

Initial driver classes:

```text
kernel_builtin
  Context, artifact, evidence, audit, and task tools.

filesystem
  Read, write, replace, delete, patch, diff, metadata.

shell
  Allowlisted command execution.

git
  Diff, status, branch, commit metadata, patch operations.

mcp
  Adapter for MCP servers.

browser
  Browser automation and screenshots.

external_api
  SaaS and enterprise APIs.
```

MCP is a driver class, not the kernel ABI.

## 8. Evidence Types

Initial evidence types:

| Type | Example |
|---|---|
| `source_ref` | file path, line, document section, URL |
| `diff_ref` | patch id, git diff, artifact version |
| `command_log` | command, exit code, stdout, stderr |
| `test_result` | test command, pass/fail, log |
| `benchmark_result` | config, metrics, environment |
| `review_finding` | reviewer, severity, artifact, line |
| `approval_record` | approver, decision, scope |
| `runtime_trace` | event ids, state transitions |
| `screenshot` | UI state proof |
| `external_reference` | web/API result with timestamp |

Evidence MUST carry provenance and timestamp.

## 9. Evidence Attachment

Evidence can attach to:

- goal
- task
- blackboard fact
- hypothesis
- decision
- artifact
- review finding
- verification result
- final claim

Every high-impact final claim MUST have at least one evidence attachment or be marked unverified.

## 10. Artifact Lifecycle

Artifact states:

```text
Draft
Submitted
UnderReview
ReviewFailed
NeedsRevision
Verified
Accepted
Rejected
Superseded
Archived
```

Rules:

- A patch artifact MUST include diff evidence.
- A test result artifact MUST include command evidence.
- A review artifact MUST identify the reviewed artifact version.
- A final answer artifact MUST include an evidence map.

## 11. Review Rules

Review MUST be independent.

Minimum review metadata:

```yaml
review_id: string
artifact_id: string
artifact_version: integer
reviewer_agent_id: string
focus:
  - correctness
  - edge_cases
  - regressions
  - security
  - maintainability
findings: []
verdict: accept | reject | needs_revision
evidence_ids: []
```

ReviewerAgent MUST NOT mutate the artifact under review.

## 12. Verification Rules

Verification checks whether evidence supports claims.

The final-submission verification gate MUST check:

- evidence exists
- evidence is current
- evidence matches artifact version
- test logs match stated commands
- final claims do not exceed evidence
- unsupported claims are marked

The kernel SHOULD reject final submission when required verification is missing.

## 13. Final Output Contract

A final answer submitted through `final.submit` MUST include:

```yaml
summary: string
changed_artifacts: string[]
evidence_map:
  - claim: string
    evidence_refs: string[]
unverified_claims: string[]
known_risks: string[]
tests_run: string[]
tests_not_run: string[]
approvals: string[]
```

The Reporter or Supervisor may generate natural language, but the final contract is structured.
