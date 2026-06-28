# Agent-OS Documentation Index

This directory turns the manifesto and collaboration theory into a development contract.

The documentation is intentionally split into foundation, kernel design, implementation, and decision records. Future code should be reviewed against these documents before accepting new subsystems, dependencies, or runtime behavior.

## Directory Structure

```text
docs/
  00-foundation/
    agent-os-manifesto.md
    agent-collaboration-theory.md
    architecture-principles.md
  05-research/
    agent-thread-source-study.md
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

Research documents record source study and design intake. They are not kernel contracts by themselves. Normative design documents and ADRs decide what Agent-OS actually adopts.

Kernel design documents define module boundaries, state contracts, syscalls, schemas, and runtime behavior. They are the primary reference for implementation.

Implementation documents define iteration sequence, acceptance gates, and quality requirements.

Architecture decision records capture decisions that are easy to accidentally reopen, such as whether PostgreSQL belongs in the kernel or whether the Agent Thread Runtime can be delegated to an open-source framework.

## Vocabulary

Agent-OS kernel: The runtime control plane that owns Agent Control Blocks, scheduling, state transitions, permissions, syscalls, evidence, artifacts, and audit logs.

Agent Thread: The kernel-managed execution unit. It contains LLM cognition, but is not defined by the LLM or by a third-party agent loop.

Agent Control Block: The durable control structure for an Agent Thread. The v0.1 field-level form is the Agent Thread Control Block (ATCB) defined in `docs/10-kernel-design/agent-thread-core-module.md`.

Provider System: The unified system-level module that resolves provider profiles, routing, model aliases, credentials, and normalized LLM streams for all Agent Threads.

Role Profile: The kernel-owned definition of an Agent Thread's semantic job, default policies, allowed delegation targets, and conformance family. The v0.1 core roles are Supervisor, Worker, and Reviewer; distributions may add aliases.

Execution Environment: A kernel-managed runtime instance with explicit mounts, toolchain, network policy, secret projection, and backend identity.

Scheduler Policy: The kernel-owned policy that defines queue class, priority, fairness, concurrency, retry, and budget reservation behavior.

Resource Lease: The auditable grant that gives an Agent Thread shared or exclusive use of a scarce resource such as a file, environment, provider slot, or human attention.

Budget Ledger: The durable accounting record for token, tool, wall-time, cost, model-request, or human-interrupt budgets.

Supervisor Hierarchy: The control graph rooted at `S0`. Delegated Supervisors increment the level (`S1`, `S2`, ...), and every delegation is recorded as an invocation edge.

Agent Invocation: The durable edge that records who spawned, delegated to, reviewed, or escalated to which Agent Thread, with assignment, scope, supervisor level, and profile snapshots.

Communication Profile: The creation-time policy that defines whether an Agent Thread may report to its Supervisor, post to scoped blackboard channels, or request human attention.

Memento Fragment: An immutable owner-scoped self-reminder that an Agent Thread leaves for its future self, often anchored to child completion, approval resolution, tool completion, resume, or compaction.

Syscall: A typed request from an Agent Thread to the kernel or to kernel-mediated services.

Tool Broker: The only path from Agent Threads to external tools, shell commands, MCP servers, APIs, browsers, or file mutation.

Model-Visible Tool: The function surface shown to a model inside a turn. v0.1 tools are grouped by domain: Host OS (`read_file`, `write_file`, `replace_text`, `delete_file`, `run_command`), work state, communication, agent supervision, privileged administration, and session lifecycle (`submit_final`).

Agent Hook: A kernel-scheduled control-plane action, such as periodic progress-report prompt injection into a child agent with the response routed back to Supervisor.

Typed Blackboard: The shared structured state for goals, tasks, facts, hypotheses, decisions, risks, artifacts, test results, review results, and acceptance criteria.

Evidence: A verifiable record that supports a claim, artifact, test result, review, or final answer.

Distribution: A packaged Agent-OS environment for a domain, such as software engineering, research, office automation, or SRE.

## Change Policy

Design changes that affect ABI, lifecycle, storage semantics, permission semantics, evidence semantics, or Agent Thread responsibilities MUST be recorded as ADRs.

Implementation details that do not change contracts can live in code comments, module docs, or issue descriptions.
