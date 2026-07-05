# Permission, Tool, and Evidence Model

Status: normative

Last updated: 2026-07-05

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

The current v0.4.0 model-visible tool surface is typed by domain.

Core and dynamically imported model-visible tools MUST project descriptions and
model input schemas from registered kernel `ToolDescriptor` records. Provider
adapters may convert the neutral descriptor into provider-specific syntax, but
they MUST NOT redefine core tool schemas in adapter-local static JSON.
Every built-in model-visible tool owner MUST also provide at least one
`ToolExample` in its descriptor. Examples include a description, model-visible
parameters, and an expected result. Provider adapters expose these examples in
the model-visible tool description instead of relying on separate prompt-only
instructions.

Host OS substrate tools are exactly:

```text
glob_files
grep_files
read_file
read_image
apply_patch
run_command
write_stdin
```

Ecosystem and deferred discovery tools are:

```text
load_skill
read_skill_resource
tool_search
```

Agent-OS control-plane tools are:

```text
set_goal
accomplish_goal
update_checklist
record_evidence
report_supervisor
post_blackboard
ask_human
request_permissions
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

Permission response `agent_control` actions are:

```text
approve_permission
deny_permission
```

`read_file`, `load_skill`, and `read_skill_resource` expose `offset` and
`limit` paging. `read_file` defaults to 200 lines and rejects limits above 1000.
These tools return `total_lines`, `returned_lines`, `next_offset`,
`truncated`, and `omitted_lines` so the model can continue reading without
pulling large files into context.

`read_image` is the read-only filesystem image tool. It accepts a
workspace-relative path to supported raster image formats, returns MIME type,
byte count, base64 encoding, and a `data_url`, and attaches `source_ref`
evidence. Provider adapters project successful `read_image` results as image
inputs only when the selected model alias has `image_input=true`; text-only
models do not see the tool, and forced calls fail through the runtime
capability guard. SVG remains a text file concern for `read_file`.

`load_skill` and `read_skill_resource` are ecosystem tools. `load_skill`
returns the imported `SKILL.md` body for a named skill. `read_skill_resource`
reads a skill-root-relative resource path and MUST reject absolute paths,
parent-directory traversal, and canonical paths outside the skill root.

`apply_patch` is the only Host OS workspace mutation tool. Each call MUST
describe exactly one file operation. Update hunks use `@@`; changed lines use
`-old` and `+new`, while unchanged context may be either plain lines or lines
prefixed with one space. The parser treats plain non-marker lines inside update
hunks as context and still rejects no-op hunks.

Local stdio MCP tools are registered as dynamic model-visible tools named
`mcp__server__tool`. Their `ToolDescriptor` comes from the kernel registry and
uses driver class `mcp`. MCP authorization is controlled by driver class and
resource scope, especially `mcp:<server>:<tool>`, because the exact model tool
names are discovered at runtime. Dynamic MCP descriptors use the same lifecycle
policy ABI as built-in tools: foreground wait cap, kernel-worker continuation,
managed text output limits, and orphan-running recovery are descriptor data
rather than adapter-local behavior.

`set_goal` and `agent_control` require `security_level <= 1` and explicit tool authority. S2+ agents MUST NOT gain these tools through permission grants. `request_permissions` remains available to lower-level child agents when their permission set allows it; it records a durable parent-directed request and does not execute the requested operation. `accomplish_goal` is visible to execution agents and marks the caller's local goal accomplished before final submission. `submit_final` is a model-visible lifecycle action, not a filesystem driver, and MUST execute through the Tool Broker as the last tool call in a session. Privileged `agent_control` actions require explicit Supervisor permission and MUST NOT appear in normal ProducerAgent tool views.

`wait_agent` is not a core tool. Agent progress is handled through status reads, output windows, lifecycle events, and `agent_control(action=set_hook)`.

Tool Broker responsibilities:

- normalize tool metadata
- collect core built-in descriptors from the per-tool owner modules under the
  kernel tool registry
- project model tool schemas from registered `ToolDescriptor` records for core
  and dynamically imported tools
- validate input schema
- check capability
- check effective permission set, including tool name and driver class
- check ecosystem resource scopes such as `instruction:*`, `skill:<name>`,
  `skill_file:<name>:*`, and `mcp:<server>:<tool>`
- enforce S-level hard gates for `agent_control` and `set_goal`
- check resource scope
- check risk level
- check environment attachment and writable lease state where applicable
- check budget admission
- enforce descriptor-declared lifecycle policy for foreground waits,
  background continuation, output management, and recovery
- request approval when required
- execute through the registered tool owner or dynamic MCP driver
- enforce the descriptor foreground wait cap for every tool invocation
- return `Running` with `tool_call_id` when a tool exceeds the foreground wait
  cap while it continues in a background worker
- capture bounded stdout, stderr, return code, and structured output
- redact secrets
- attach evidence
- record audit event

Agent Threads MUST NOT call tools directly.

Tool outputs MUST be bounded at the tool layer. `apply_patch` returns operation,
path, byte counts, hunk or replacement counts, hashes, and a small preview
instead of full before/after file bodies. `run_command`, MCP raw results,
communication payloads, evidence inline content, and agent output windows MUST
apply explicit budgets before they enter model-visible context. Provider message
formatters may still apply defensive truncation, but they are not the primary
context safety boundary.

## 6.1 Agent Supervision Tool Semantics

`agent_control(action=start)` MUST create durable agent state and an invocation edge before launching provider work. The `start` action MUST include `payload.goal` as the child local goal and MAY set multiple control-plane properties atomically, including timeout policy, output policy, success/failure criteria, and initial hooks.

The `agent_control` input shape is action-specific, but all actions share this envelope:

```yaml
action: start | status | output | set_hook | send | resume | stop | set_timeout | export_trace | kill | delete_session | purge_state | approve_permission | deny_permission
agent_id: string | null
thread_id: string | null
idempotency_key: string
payload: object
```

`agent_control(action=output)` accepts `payload.cursor` and `payload.limit` to
read a bounded provider-stream window for the target thread. When
`payload.tool_call_id` is present, it returns the matching tool invocation plus
any active background worker metadata for that call id. For process tools such
as `run_command`, that response includes bounded rolling stdout/stderr previews,
byte counts, and truncation flags so a supervisor can inspect compile or test
progress before the command exits. This is not special to `agent_control` or
`run_command`: every tool result with managed text fields is attached to the
same tool-output manager. By default, a `tool_call_id` lookup returns
`payload.new=200` lines after the supplied cursor. The caller may request
`payload.head`, `payload.tail`, or `payload.new` line limits, or set
`payload.full=true` with `payload.offset` and `payload.limit` for line paging
over the complete spooled field. `payload.cursor` may be a byte offset or an
object such as `{stdout: 1200, stderr: 40}`; `payload.new` returns output after
that cursor and advances `next_cursor`, so polling does not need to resend
output the supervisor has already seen. Hard byte caps still apply to every
returned window.

A started agent record includes:

```yaml
agent_id: string
thread_id: string
invocation_id: string
security_level: integer
status: string
workdir: string
timeout_seconds: integer
session_id: string | null
output_handle: string
```

Example `start` payload:

```yaml
role_profile_id: role_producer
goal: "Inspect storage replay gaps and report findings."
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

## 6.2 Parent Permission Requests

Agent-OS uses a subset-grant pattern for permission requests: a child asks for a concrete permission profile, the parent may grant only a subset, omitted fields are denied, and the grant is scoped to the current turn or session.

`request_permissions` input:

```yaml
reason: string
scope: turn | session
permissions:
  max_risk_level: integer
  allowed_syscalls: string[]
  resource_scopes: string[]
  allowed_tool_names: string[]
  allowed_tool_driver_classes: string[]
  approval_required_above: integer
  requires_evidence_for: string[]
```

The kernel records `PermissionRequested` with requester, direct parent approver, requested permissions, session id, optional turn id, reason, and pending status. The parent responds with `agent_control(action=approve_permission)` or `agent_control(action=deny_permission)`. Approved permissions are intersected with both the original request and the parent effective permission set before the kernel records `PermissionGranted`.

Session grants apply to later tool invocations in the same child session. Turn grants apply only while the recorded turn is still active. Grants never override S-level hard gates.

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

Built-in tool descriptors declare whether a successful tool result should attach
evidence. Workspace reads attach `source_ref`, patches attach `diff_ref`,
commands attach `command_log`, control-plane state and communication tools attach
`runtime_trace`, and permission requests attach `approval_record`.
`record_evidence` creates the requested evidence directly and `submit_final`
does not create a second self-referential evidence record.

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

A final answer submitted through `submit_final` MUST include:

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
