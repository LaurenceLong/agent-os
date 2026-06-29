# S-Level Permissions Post-Fix Audit

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree at pre-fix audit: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status after the fix: dirty. The tree already contained broad
  uncommitted changes before this work; this audit records the S-level
  permission implementation on top of that current tree.

## Implemented Fixes

- Replaced role-specific `supervisor_level` ABI fields with universal
  `security_level`. Human authority is implicit S0, root agents are S1, and
  every child increments the parent level.
- Replaced flattened permission-profile fields with a typed `PermissionSet`
  that includes risk, syscall, resource scope, tool name, tool driver class,
  approval threshold, and evidence requirements.
- Added creation-time `effective_permissions_snapshot` to
  `AgentControlBlock`, durable `PermissionRequest`, and durable
  `PermissionGrant`.
- Added kernel permission-set operations for subset checks, intersections,
  dynamic session/turn grant overlays, and hard S-level control-plane gates.
- Enforced effective permissions in capability grants, syscall authorization,
  and Tool Broker dispatch. `agent_control` and `set_goal` now require
  `security_level <= 1` plus tool permission.
- Added model-visible `request_permissions` and parent responses through
  `agent_control(action=approve_permission|deny_permission)`.
- Updated OpenAI/Anthropic tool visibility to filter by permission snapshot and
  S-level, added prompt guidance, parser mappings, and result reconstruction.
- Updated current contract docs and README to describe S-level hierarchy,
  permission requests, subset grants, and turn/session grant scope.

## Focused Tests Added or Updated

- S-level hierarchy and invocation replay now assert root S1, delegated S2,
  and nested worker S3.
- Security conformance covers S2+ denial for `agent_control` and `set_goal`.
- Security conformance covers child `request_permissions`, parent approval,
  session grant success, denial no-op, and turn grant expiry.
- Tool-broker and adapter tests cover the `request_permissions` core tool,
  permission-filtered model-visible tools, and new `agent_control` response
  actions.

## Validation Results

- `cargo fmt --all`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
  - CLI unit tests: 13 passed.
  - Conformance integration tests: 72 passed.
  - Kernel tests: 8 passed across unit/integration targets.
  - Store tests: 5 passed.
  - Sys tests: 1 passed.
  - Thread tests: 28 passed, 10 live LLM e2e tests ignored by credential policy.
  - Doctests: all empty targets passed.

## Forward-Only Notes

- No compatibility layer was kept for `supervisor_level` or flattened
  permission-profile JSON shapes.
- Grants never override the S-level hard gate. A parent can grant tool names and
  driver classes, but S2+ agents still cannot execute `agent_control` or
  `set_goal`.
- Turn grants are active only while the recorded turn remains in an active
  status; returning the thread to `Ready` completes the turn for grant purposes.

## Remaining Gaps

- Root-agent permission requests record a pending request with no modeled human
  approver. Human-facing approval UX remains a future layer over the same
  durable request/grant event contract.
- Runtime tool visibility currently uses the thread permission snapshot; dynamic
  grants are enforced by the kernel even when a future runtime view may choose
  to surface newly granted tools in the next model turn.
