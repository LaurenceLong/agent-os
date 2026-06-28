# Role and Profile System

Status: normative

Last updated: 2026-06-26

## 1. Purpose

Agent-OS MUST treat roles and runtime profiles as kernel-owned control-plane state.

An Agent Thread may carry a `role` label in its control block, but the label alone is not enough to define behavior, authority, communication rights, or isolation boundaries.

The system therefore needs a dedicated Role and Profile System that resolves the effective runtime contract for every Agent Thread.

## 2. Why This Is Not Thread-Local

The following concerns MUST NOT be left inside Agent Thread implementation:

- what a role is allowed to do
- which child roles it may spawn
- which permission ceiling it runs under
- which sandbox and execution envelope it requires
- which provider defaults it inherits
- which scheduling class it belongs to
- whether a distribution-defined role is still conformant with kernel semantics

If these rules live inside prompts or per-thread code, the system becomes impossible to audit and distributions will silently diverge.

## 3. Responsibilities

The Role and Profile System owns:

- canonical role definitions
- role-family classification used by conformance tests
- binding of role defaults to permission, sandbox, provider, and scheduler policies
- versioning and supersession of profiles
- role compatibility rules for child thread creation
- Supervisor level rules for delegated Supervisor creation
- invocation edge creation for delegation, worker assignment, review request, and escalation
- escalation policy for restricted actions
- effective profile resolution when an Agent Thread is created

The Agent Thread Runtime consumes resolved bindings. It does not invent them.

## 4. Design Model

Agent-OS distinguishes several layers:

### 4.1 Role Profile

A Role Profile defines the semantic job of an Agent Thread.

Examples:

- `SupervisorAgent`
- `WorkerAgent`
- `ReviewerAgent`

The v0.1 core role set is `SupervisorAgent`, `WorkerAgent`, and `ReviewerAgent`.

A Role Profile may also be distribution-specific, but it MUST still declare a conformance family such as `producer`, `reviewer`, `operator`, or `custom`. Distribution workflow step labels are policy-pack conventions, not kernel-required roles.

### 4.2 Permission Profile

A Permission Profile defines the maximum syscall and resource authority a thread may exercise before capability and approval checks are applied.

### 4.3 Sandbox Profile

A Sandbox Profile defines the operating envelope:

- read-only vs workspace-write
- isolated worktree vs shared workspace
- network policy
- process isolation backend
- secret projection policy

### 4.4 Provider Defaults

Provider defaults MAY be attached to a role so the system can route role classes toward suitable models, but the actual stream MUST still be resolved by the Provider System.

### 4.5 Scheduler Class

Scheduling class is not a prompt hint. It is a kernel policy binding that tells the scheduler how to prioritize, defer, or quarantine the thread.

## 5. Effective Binding

When the kernel creates an Agent Thread, it MUST resolve an immutable-at-turn-start effective binding:

```yaml
effective_binding:
  role_profile_id: string
  permission_profile_id: string
  sandbox_profile_id: string
  provider_profile_id: string | null
  scheduler_policy_id: string | null
reasoning_profile: string | null
communication_profile_id: string
supervisor_level: integer | null
invocation_id: string | null
resolved_at: string
revision: integer
```

Rules:

- a thread MUST NOT widen its own binding
- a capability token MUST NOT exceed the bound Permission Profile
- a sandbox escalation MUST be treated as a new kernel decision
- a profile update MUST create a new binding revision
- active turns continue with the binding snapshot they started with
- `supervisor_level` MUST be set for SupervisorAgent threads
- delegated Supervisors increment the caller Supervisor level by one
- every binding created by delegation MUST reference an invocation edge

## 6. Delegation and Child Thread Creation

Child creation MUST go through the Role and Profile System and Agent Control.

Inputs:

- invoking Supervisor role profile
- requested child role
- caller Supervisor level when the caller is a Supervisor
- task risk level
- scheduler policy
- distribution policy pack

Checks:

- whether the invoking Supervisor may assign that child role
- whether the requested role is legal in the current distribution
- whether review independence would be broken
- whether the requested sandbox and permission envelope is allowed
- whether a delegated Supervisor would have the correct next level

The Supervisor MAY propose a child role. The kernel decides whether it is admissible.

The kernel MUST persist an invocation edge for every accepted delegation or assignment:

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

Rules:

- The top-level Supervisor is `S0`.
- A delegated Supervisor created by `S<N>` is `S<N+1>`.
- WorkerAgent and ReviewerAgent threads have `supervisor_level: null` but still reference the invocation edge that assigned them.
- Invocation edges are append-only. Corrections create new edges or supersession events.
- The invocation graph is used for replay, audit, cancellation, and responsibility tracing.

## 7. Logical Schemas

### 7.1 RoleProfile

```yaml
role_profile_id: string
status: Active | Superseded | Revoked
name: string
role_family: producer | reviewer | operator | custom
purpose: string
default_permission_profile_id: string
default_sandbox_profile_id: string
default_provider_profile_id: string | null
default_scheduler_policy_id: string | null
allowed_child_role_profile_ids: string[]
required_review_mode: none | independent | dual
escalation_policy: object | null
distro_scope: core | distribution
created_at: string
updated_at: string
superseded_by: string | null
```

### 7.2 PermissionProfile

```yaml
permission_profile_id: string
status: Active | Superseded | Revoked
name: string
max_risk_level: integer
allowed_syscalls: string[]
resource_scopes: string[]
denied_tool_classes: string[]
approval_required_above: integer
requires_evidence_for: string[]
created_at: string
updated_at: string
superseded_by: string | null
```

### 7.3 SandboxProfile

```yaml
sandbox_profile_id: string
status: Active | Superseded | Revoked
name: string
filesystem_mode: read_only | workspace_write | isolated_worktree | temp_only | custom
network_mode: off | allowlist | full
process_backend: native | job_object | container | vm | remote_worker
secret_policy: none | scoped_handles | injected_ephemeral
toolchain_profile_id: string | null
mount_policy: object | null
created_at: string
updated_at: string
superseded_by: string | null
```

## 8. Kernel Invariants

The Role and Profile System MUST enforce:

1. role labels do not grant authority by themselves
2. profile widening is impossible from inside the Agent Thread
3. review independence is expressed in role policy, not only in prompts
4. distribution-defined roles remain mapped to core conformance families
5. profile supersession never deletes historical bindings
6. Supervisor levels and invocation edges are kernel state, not prompt text

## 9. Relationship to Other Subsystems

- Provider System resolves model streams after role defaults are applied.
- Communication Kernel enforces the Communication Profile assigned at creation time.
- Execution Environment System provisions environments that satisfy the Sandbox Profile.
- Scheduler and Resource Arbitration uses Scheduler Policy and role family when ranking work.
- Permission Kernel intersects capability tokens with the effective Permission Profile.

## 10. First Implementation Target

The first production implementation SHOULD ship core Role Profiles for:

- SupervisorAgent
- ReviewerAgent
- WorkerAgent

Distributions MAY add aliases or additional roles only after they map cleanly onto the core conformance families and preserve the invocation graph.
