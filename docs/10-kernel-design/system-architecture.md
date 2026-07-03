# System Architecture

Status: normative

Last updated: 2026-07-03

## 1. Architecture Summary

Agent-OS is structured as a microkernel-style runtime with dedicated Agent Threads.

```text
Human / API / CLI
      |
      v
Agent-OS App Server
      |
      v
Agent-OS Host
      |
      v
Agent-OS Kernel
      |
      +-- Agent Control Block Manager
      +-- Scheduler and Resource Arbitration
      +-- Role and Profile System
      +-- Task DAG Manager
      +-- Typed Blackboard
      +-- Execution Environment System
      +-- Communication Kernel
      +-- Provider System
      +-- Context Manager
      +-- Memento Manager
      +-- Memory Manager
      +-- Tool Broker
      +-- Permission Kernel
      +-- Evidence Store
      +-- Artifact Store
      +-- Review Runtime
      +-- Conflict Resolver
      +-- Audit Log
      |
      v
Agent Thread Runtime
      |
      +-- SupervisorAgent
      +-- ProducerAgent
      +-- ReviewerAgent
      |
      v
Drivers and Services
      |
      +-- LLM providers
      +-- MCP servers
      +-- shell tools
      +-- file systems
      +-- git providers
      +-- browsers
      +-- databases
      +-- enterprise APIs
```

## 2. Core Packages

Current Rust workspace package layout is:

```text
crates/
  agent-os-sys/
  agent-os-store/
  agent-os-store-sqlite/
  agent-os-kernel/
  agent-os-thread/
  agent-os-config/
  agent-os-ecosystem/
  agent-os-distro/
  agent-os-app-server/
  agent-os-host/
  agent-os-cli/
  agent-os-conformance/

distros/
  software-engineering/

docs/
```

The first implementation language for kernel crates is Rust.

TypeScript and Python SHOULD be SDK and distribution languages, not kernel implementation languages.

## 3. Kernel Responsibilities

The kernel MUST own all state transitions that affect task correctness, auditability, or permissions.

### 3.1 Agent Control Block Manager

Owns Agent Thread identity and lifecycle.

Responsibilities:

- create Agent Control Blocks
- persist Agent Control Blocks
- validate state transitions
- bind Agent Threads to tasks
- bind capabilities to Agent Threads
- record checkpoints
- record termination reasons

### 3.2 Scheduler and Resource Arbitration

Schedules work at cooperative boundaries and arbitrates scarce resources.

Boundaries include:

- before LLM call
- after LLM call
- before tool invocation
- after tool invocation
- before artifact commit
- before approval request
- before final submission
- after task state transition

Additional responsibilities:

- assign queue classes and priority
- arbitrate file, workspace, environment, provider-slot, and human-attention resources
- reserve and account for budgets
- detect starvation and priority inversion
- suspend, defer, or quarantine work when policy requires

The scheduler MUST NOT attempt to preempt an LLM call mid-generation in v0.1.

### 3.3 Role and Profile System

Owns kernel-level role semantics and runtime profile binding.

Responsibilities:

- define canonical Role Profiles
- define Permission Profiles and Sandbox Profiles
- map distribution roles to core conformance families
- resolve effective bindings at Agent Thread creation time
- control which child roles may be spawned
- version profile supersession without erasing historical bindings

Role labels alone MUST NOT grant authority.

### 3.4 Task DAG Manager

Owns task decomposition and dependency readiness.

Responsibilities:

- register tasks
- track dependencies
- block dependent tasks until inputs are ready
- prevent stale review or stale test execution
- detect dependency cycles
- expose ready queues to the scheduler

### 3.5 Typed Blackboard

Stores structured shared state.

Initial blackboard sections:

- goal
- constraints
- known_facts
- hypotheses
- decisions
- open_questions
- tasks
- risks
- artifacts
- evidence
- test_results
- review_results
- final_acceptance_criteria

All entries MUST carry provenance.

### 3.6 Execution Environment System

Owns kernel-managed runtime environments and their leases.

Responsibilities:

- resolve environment templates
- provision local, isolated, container, VM, or remote-worker environments
- attach workspace, artifact, and temp mounts
- apply sandbox, network, and secret projection policy
- record environment identity for audit and replay
- grant and revoke environment leases for Agent Threads

Agent Threads MUST NOT silently pick their own backend, writable mounts, or secret projection behavior.

### 3.7 Communication Kernel

Controls Agent Thread communication routes.

Responsibilities:

- enforce creation-time Communication Profiles
- route messages to Supervisor, scoped blackboard channels, or humans
- validate message schemas
- enforce blackboard scope and channel policy
- enforce human attention budget
- decide whether delivery triggers a receiver turn
- record delivery, rejection, and audit events

Agent Threads MUST NOT communicate directly. They communicate through kernel syscalls.

### 3.8 Provider System

Controls system-wide provider configuration and normalized LLM streaming.

Responsibilities:

- resolve provider profiles
- resolve model aliases
- apply routing policy
- resolve credentials
- open normalized stream sessions
- emit usage, cost, retry, and failure events
- isolate provider-specific SDK and API behavior behind adapters

Agent Threads must obtain streams through Provider System. They must not construct provider SDK clients directly.

### 3.9 Context Manager

Controls semantic working memory.

Responsibilities:

- load scoped context
- track loaded files and documents
- record context freshness
- maintain summaries
- compact context
- detect stale context
- detect context pollution
- support context versioning

Agent Threads MUST request context through the kernel. They MUST NOT silently inherit all global context.

### 3.10 Memento Manager

Controls immutable owner-scoped reminders used by Agent Threads to resume correctly after delegation, waiting, compaction, and callbacks.

Responsibilities:

- create Memento Fragment drafts
- arm Memento Fragments and freeze content hash
- anchor fragments to child completion, tool completion, approval resolution, review callbacks, verification callbacks, resume, compaction, or time
- trigger fragments when anchor conditions happen
- project triggered fragments only to the owner Agent Thread
- enforce that child Agent Threads cannot read or mutate parent fragments
- consume, supersede, expire, or invalidate fragments through kernel transitions
- preserve fragment lifecycle in the audit trail

Memento Fragments are not child assignments and are not long-term memory.

### 3.11 Memory Manager

Controls long-term memory visibility and writes.

Responsibilities:

- namespace memory
- enforce read permissions
- require provenance for writes
- support proposed writes before commit
- mark stale or contested memory
- prevent unverified conclusions from becoming durable memory

### 3.12 Tool Broker

The Tool Broker is the only supported path to external side effects.

Responsibilities:

- discover tools
- normalize tool schemas
- enforce capability checks
- execute tools through drivers
- record inputs and outputs
- attach evidence when applicable
- apply rate limits
- enforce idempotency policies
- provide rollback metadata when available

### 3.13 Permission Kernel

Evaluates whether an Agent Thread may perform an action.

Inputs:

- agent identity
- role binding
- permission profile
- sandbox profile
- capability token
- resource scope
- requested syscall
- risk level
- task state
- approval state
- budget state
- environment state

Outputs:

- allow
- deny
- require_approval
- require_more_evidence
- require_lower_risk_plan

Additional responsibilities:

- bind approval scope to requested action and resource
- reject approval reuse outside scope
- intersect capability tokens with resolved Permission Profiles
- require compatible environment and lease state for protected operations

### 3.14 Evidence Store

Stores evidence metadata and links to evidence blobs.

Evidence MUST be immutable after commit. Corrections are new evidence records that supersede or invalidate older records.

### 3.15 Artifact Store

Stores artifacts and artifact metadata.

Artifacts include:

- plans
- patches
- test logs
- review reports
- benchmark outputs
- analysis notes
- final answers
- memory proposals

### 3.16 Review Runtime

Enforces independent review rules.

It MUST know:

- artifact owner
- reviewers
- review scope
- required evidence
- accepted findings
- rejected findings
- unresolved risks

### 3.17 Conflict Resolver

Handles contradictory claims, failed reviews, stale artifacts, resource conflicts, and policy conflicts.

Resolution priority:

1. current tool evidence
2. current file state
3. project rules
4. explicit user instruction
5. durable decision records
6. human escalation

### 3.18 Audit Log

Records who did what, why, with which permission, against which resource, and with what result.

Audit events MUST be append-only.

## 4. Control Plane and Data Plane

Control plane:

- ACB state
- role bindings and profile versions
- task DAG
- scheduling queues and policies
- budget ledgers
- resource and environment leases
- permissions
- policies
- blackboard
- evidence metadata
- artifact metadata
- audit events

Data plane:

- environment provisioning
- tool execution
- file reads and writes
- shell command execution
- model calls
- browser actions
- external API calls
- artifact blob storage

The control plane MUST stay authoritative even when data-plane services fail.

## 5. Distribution Boundary

The kernel is not a distribution.

A distribution packages:

- domain agents
- role packages
- prompts
- policy packs
- environment templates
- tool drivers
- UI workflows
- storage deployment
- model defaults
- operational dashboards

The first official distribution SHOULD be `software-engineering`.
