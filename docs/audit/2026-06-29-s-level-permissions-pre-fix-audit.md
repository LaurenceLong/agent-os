# S-Level Permissions Pre-Fix Audit

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status before this audit: dirty. Existing modified files span
  workspace configuration, CLI/provider configuration, kernel, thread runtime,
  conformance tests, docs, and prior audit records. This change treats that
  work as pre-existing user work and edits only the S-level permissions scope.

## Audit Scope

This audit covers the current permission and control-plane implementation before
adding Linux-like S-level permissions and parent-approved permission requests.
The scope includes ABI structs, kernel spawn and tool authorization, built-in
tool schemas, model-visible tool filtering, conformance tests, and design docs.

## Current-Contract Findings

1. `AgentControlBlock` stores `supervisor_level: Option<u32>`, and
   `AgentInvocation` stores caller/callee supervisor levels. The value is only
   assigned to `SupervisorAgent`; Worker and Reviewer threads have `None`, so
   the model cannot express the intended universal S-level hierarchy.
2. `agent_control` and `set_goal` are currently gated mostly by role/risk/tool
   visibility. The kernel does not enforce a universal `security_level <= 1`
   control-plane boundary.
3. `PermissionProfile` defines syscall/resource/risk limits, but it does not
   model allowed tool names, allowed driver classes, or a reusable permission
   set snapshot that can be granted to children.
4. Tool invocation authorization checks capability and syscall permissions
   before dispatch, but the Tool Broker does not enforce tool-name or
   driver-class authority against the caller's effective permissions.
5. There is no model-visible `request_permissions` tool, no durable
   permission-request projection, and no parent approval path that grants only a
   subset of a child's request.
6. The OpenAI tool adapter currently filters Supervisor-only tools by role name
   instead of S-level and does not expose permission request semantics to
   lower-level child agents.

## Future-Roadmap Findings

- The first implementation should keep scope to deterministic kernel-owned
  permission grants with `Turn` and `Session` scope. Rich UI waiters,
  expiration policies, revocation UX, and external human approval routing can be
  layered later if they remain consistent with the same durable event contract.

## Validation Already Run

- Static inspection with `rg` for `supervisor_level`, permission profiles,
  tool broker paths, `agent_control`, `set_goal`, and tool schemas.
- No tests or builds have been run before this pre-fix audit.

## Intended Fix Scope

- Replace supervisor-level ABI fields with universal `security_level` fields.
- Add `PermissionSet`, `PermissionGrantScope`, permission requests, and
  permission grants to the shared ABI.
- Compute S-level on spawn, enforce parent subset grants, and persist an
  effective permission snapshot on each thread.
- Enforce tool-name, driver-class, risk, syscall, and resource authority in the
  kernel before tool driver dispatch.
- Add `request_permissions` and parent approval/denial via `agent_control`.
- Update model-visible tool filtering, prompt text, conformance coverage, and
  design docs for the new forward-only contract.
