# Agent-OS Documentation Index

This directory turns the manifesto, collaboration theory, implementation
snapshot, and decision records into a development contract.

The documentation is intentionally split into foundation, kernel design,
implementation, and decision records. Current code and future changes should be
reviewed against these documents before accepting new subsystems, dependencies,
or runtime behavior.

## Directory Structure

```text
docs/
  00-foundation/
    agent-os-manifesto.md
    agent-collaboration-theory.md
    architecture-principles.md
  10-kernel-design/
    system-architecture.md
    overall-architecture-mermaid.md
    agent-thread-runtime.md
    agent-thread-core-module.md
    role-and-profile-system.md
    execution-environment-system.md
    scheduler-and-resource-arbitration.md
    provider-system.md
    agent-thread-communication.md
    memento-fragments.md
    kernel-data-model.md
    kernel-abi-and-syscalls.md
    state-storage-and-replay.md
    permission-tool-evidence-model.md
  20-implementation/
    production-roadmap.md
    conformance-and-quality.md
    swe-bench-lite-private-benchmark.md
  30-decisions/
    ADR-0001-agent-os-is-a-microkernel-runtime.md
    ADR-0002-postgresql-is-a-storage-driver.md
    ADR-0003-agent-thread-runtime-is-proprietary-core.md
    ADR-0004-agent-thread-core-module-source-informed-clean-room.md
    ADR-0005-memento-fragments-are-owner-self-reminders.md
    ADR-0006-agent-thread-communication-is-capability-scoped.md
    ADR-0007-provider-system-is-global-control-plane.md
    ADR-0008-thread-adjacent-concerns-are-kernel-subsystems.md
    ADR-0009-v0-1-core-surface-convergence.md
```

## Document Classes

Foundation documents define non-negotiable principles. They should change rarely.

The English versions under `docs/00-foundation/` are the canonical foundation documents used by the implementation documentation. The original Chinese source documents may be archived outside the root when desired.

Kernel design documents define module boundaries, state contracts, syscalls, schemas, and runtime behavior. They are the primary reference for implementation.

Implementation documents define iteration sequence, acceptance gates, and quality requirements.

Architecture decision records capture decisions that are easy to accidentally reopen, such as whether PostgreSQL belongs in the kernel or whether the Agent Thread Runtime is native Agent-OS core infrastructure.

## Vocabulary

Agent-OS kernel: The runtime control plane that owns Agent Control Blocks, scheduling, state transitions, permissions, syscalls, evidence, artifacts, and audit logs.

Agent Thread: The kernel-managed execution unit. It contains LLM cognition, but is not defined by the LLM or by a third-party agent loop.

Agent Control Block: The durable control structure for an Agent Thread. The current field-level form is the Agent Thread Control Block (ATCB) defined in `docs/10-kernel-design/agent-thread-core-module.md`.

Provider System: The unified system-level module that resolves provider profiles, routing, model aliases, credentials, and normalized LLM streams for all Agent Threads.

Role Profile: The kernel-owned definition of an Agent Thread's semantic job, default policies, allowed delegation targets, and conformance family. The current core roles are Supervisor, Producer, and Reviewer; distributions may add aliases.

App Server: The JSONL protocol gate that exposes thread start/read/list/archive, thread fork/rollback/compact, paged thread turn and timeline reads, turn start/steer/interrupt, stats, notifications, automation, and task bundle export over typed `AppRequest` envelopes. It owns app-facing response shapes such as the `thread/read` projection.

Host Layer: The `agent-os-host` service boundary. It opens the SQLite-backed kernel store, combines kernel/store/runtime/app-server components, owns runtime job records and background workers, and starts configured Agent Thread Runtime jobs through the `agent-os-hostd` process.

Execution Environment: A kernel-managed runtime instance with explicit mounts, toolchain, network policy, secret projection, and backend identity.

Scheduler Policy: The kernel-owned policy that defines queue class, priority, fairness, concurrency, retry, and budget reservation behavior.

Resource Lease: The auditable grant that gives an Agent Thread shared or exclusive use of a scarce resource such as a file, environment, provider slot, or human attention.

Budget Ledger: The durable accounting record for token, tool, wall-time, cost, model-request, or human-interrupt budgets.

Security Hierarchy: The control graph rooted at implicit human `S0`. Root agents start at `S1`, every child increments the level, and every delegation is recorded as an invocation edge.

Agent Invocation: The durable edge that records who spawned, delegated to, reviewed, or escalated to which Agent Thread, with child goal, scope, security level, and profile snapshots.

Communication Profile: The creation-time policy that defines whether an Agent Thread may report to its Supervisor, post to scoped blackboard channels, or request human attention.

Memento Fragment: An immutable owner-scoped self-reminder that an Agent Thread leaves for its future self, often anchored to child completion, approval resolution, tool completion, resume, or compaction.

Syscall: A typed request from an Agent Thread to the kernel or to kernel-mediated services.

Tool Broker: The only path from Agent Threads to external tools, shell commands, MCP servers, APIs, browsers, or file mutation.

Model-Visible Tool: The function surface shown to a model inside a turn. v0.3 tools are grouped by domain: Host OS (`glob_files`, `grep_files`, `read_file`, `read_image`, `apply_patch`, `run_command`, `write_stdin`), ecosystem and deferred discovery (`load_skill`, `read_skill_resource`, `tool_search`), work state, communication, agent supervision, privileged administration, and session lifecycle (`submit_final`).

Agent Hook: A kernel-scheduled control-plane action, such as periodic progress-report prompt injection into a child agent with the response routed back to Supervisor.

Typed Blackboard: The shared structured state for goals, tasks, facts, hypotheses, decisions, risks, artifacts, test results, review results, and acceptance criteria.

Evidence: A verifiable record that supports a claim, artifact, test result, review, or final answer.

Distribution: A packaged Agent-OS environment for a domain, such as software engineering, research, office automation, or SRE. In Rust, `agent-os-distro` owns distribution prompt and workflow builders; `agent-os-thread` consumes prepared runtime inputs.

## Change Policy

Design changes that affect ABI, lifecycle, storage semantics, permission semantics, evidence semantics, or Agent Thread responsibilities MUST be recorded as ADRs.

Implementation details that do not change contracts can live in code comments, module docs, or issue descriptions.
