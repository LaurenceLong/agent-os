# Execution Environment System

Status: normative

Last updated: 2026-06-25

## 1. Purpose

Agent-OS MUST treat execution environments as kernel-managed resources.

An Agent Thread does not "just run in a shell". It runs inside an attached environment whose workspace mounts, toolchain, network policy, secret projection, and isolation backend are part of the system contract.

## 2. Why This Is Not Thread-Local

If execution state lives only inside thread-local assumptions such as current directory, inherited process state, or ad hoc tool setup, the system cannot answer basic production questions:

- which environment produced this artifact
- which mounts were writable
- whether the network was enabled
- which secrets were projected
- whether the run was local, containerized, virtualized, or remote
- whether a replay is materially equivalent

Execution environments therefore belong to the kernel control plane.

## 3. Responsibilities

The Execution Environment System owns:

- environment templates
- environment provisioning and teardown
- workspace and artifact mounts
- toolchain projection
- network policy attachment
- secret projection handles
- attach and detach leases for Agent Threads
- environment reuse policy
- environment identity in audit and replay records

## 4. Backends

v0.1 SHOULD support a unified abstraction over these backend classes:

- local process
- isolated worktree
- container
- VM
- remote worker

The backend is an implementation detail for drivers, but it is not invisible. The kernel MUST record which backend class was actually used.

## 5. Environment Model

### 5.1 Template

A template describes what kind of environment is needed.

Examples:

- read-only repository explorer
- workspace writer
- test runner with temporary outputs
- network-enabled research worker
- release signer

### 5.2 Instance

An environment instance is a concrete provisioned runtime with stable identity.

### 5.3 Lease

A thread executes against an environment through a lease. The lease is the auditable attachment between Agent Thread and environment instance.

## 6. Logical Schemas

### 6.1 ExecutionEnvironment

```yaml
environment_id: string
status: Requested | Provisioning | Ready | Attached | Draining | Terminated | Failed
backend_type: local_process | isolated_worktree | container | vm | remote_worker
template_name: string
sandbox_profile_id: string
host_id: string | null
workspace_mounts: object[]
artifact_mounts: object[]
toolchain_profile_id: string | null
network_policy_id: string | null
secret_projection_id: string | null
reuse_policy: exclusive | task_scoped | pooled
created_at: string
updated_at: string
terminated_at: string | null
```

### 6.2 EnvironmentLease

```yaml
environment_lease_id: string
environment_id: string
agent_id: string
thread_id: string
task_id: string
attach_mode: read_only | workspace_write | exclusive
status: Active | Released | Expired | Revoked
started_at: string
expires_at: string | null
released_at: string | null
```

## 7. Provisioning Flow

The normative flow is:

1. Role and Profile System resolves the required Sandbox Profile.
2. Scheduler decides whether environment acquisition is allowed now.
3. Execution Environment System provisions or reuses a compatible environment.
4. Kernel records the environment instance and lease.
5. Tool Broker executes work through that attached environment.

The Agent Thread MUST NOT directly pick a backend implementation or mount writable paths on its own.

## 8. Workspace and Secret Rules

Rules:

- writable workspace access requires both a capability grant and an attached environment with writable mount policy
- secret projection MUST use handles or ephemeral injection, not durable ACB fields
- environment identity MUST be attached to tool evidence when material to reproduction
- a backend change that affects reproducibility MUST emit a durable event
- an exclusive environment lease blocks concurrent incompatible use

## 9. Replay and Recovery

Replay does not require byte-for-byte resurrection of every process, but it MUST preserve:

- the environment class
- mount and write policy
- toolchain identity where relevant
- network and secret policy class
- environment-to-thread lease history

If exact reattachment is impossible, the kernel MUST surface that fact as a replay limitation, not hide it.

## 10. Relationship to Other Subsystems

- Role and Profile System declares the Sandbox Profile requirement.
- Scheduler and Resource Arbitration decides when environments are acquired, reused, or drained.
- Tool Broker performs commands and external side effects through the attached environment.
- Permission Kernel checks whether requested environment access is within scope.
- State, Storage, and Replay must store environment identity and lease events.

## 11. First Implementation Target

The first production implementation SHOULD support:

- read-only local process environments
- isolated workspace-write environments
- test runner environments with temp output policy

Container, VM, and remote-worker backends SHOULD share the same logical contract before they are introduced.
