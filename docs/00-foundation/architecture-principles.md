# Architecture Principles

Status: normative

Last updated: 2026-06-25

## 1. Positioning

Agent-OS is a runtime kernel for agent organizations.

It is designed to manage entities whose execution is uncertain, tool-mediated, context-dependent, and goal-driven. It therefore manages resources that traditional operating systems do not manage directly:

- goals
- scoped context
- long-term memory
- tool permissions
- evidence
- artifacts
- risk
- cost
- human attention
- long-running task state

Agent-OS does not replace Linux, Windows, or macOS. It runs above them and provides agent-native scheduling, governance, recovery, and audit semantics.

## 2. Microkernel Principle

The kernel MUST remain small, strict, and durable.

The kernel owns:

- Agent Control Blocks
- task DAG state
- Agent Thread lifecycle
- typed event log
- syscall validation
- scheduling decisions
- capability checks
- resource locks
- artifact metadata
- evidence metadata
- audit records
- conflict state

The kernel MUST NOT own:

- domain-specific prompts
- vendor-specific LLM APIs
- UI workflows
- PostgreSQL-specific behavior
- cloud deployment assumptions
- business automation templates
- third-party agent loop semantics

External systems are drivers, services, or distribution components.

## 3. Agent Thread Principle

The Agent Thread is the equivalent of a thread in the Agent-OS world.

It MUST be a dedicated runtime entity designed for the kernel. It MUST NOT be an unmodified open-source agent framework hidden behind an adapter.

An Agent Thread is defined by:

- an Agent Control Block
- a role
- a goal
- a lifecycle state
- capability tokens
- scoped context visibility
- allowed syscalls
- evidence obligations
- artifact ownership rules
- yield checkpoints
- audit identity

The LLM is a cognitive coprocessor inside the Agent Thread. The LLM does not define the runtime boundary.

## 4. Evidence-First Principle

Natural language is not evidence.

The final output of an Agent-OS task MUST be derived from structured evidence:

- file references
- diffs
- command logs
- test outputs
- benchmark outputs
- review findings
- external source references
- approval records
- artifact provenance

Unsupported claims MUST either be removed, downgraded, or explicitly marked as unverified.

## 5. Supervisor-Producer-Reviewer Separation

Agent-OS MUST enforce separation of responsibility.

The producer of an artifact cannot be the sole reviewer or acceptor of that artifact.

The foundation role set is:

- SupervisorAgent: owns goals, task DAGs, delegation, permission arbitration, final acceptance, and escalation.
- ProducerAgent: produces artifacts.
- ReviewerAgent: independently reviews artifacts and verifies supporting evidence with tools.

ProducerAgent and ReviewerAgent have equivalent baseline capability. They differ by responsibility, artifact ownership, and review obligations, not by broad tool deprivation.

For software engineering work:

- A ProducerAgent creates patches, test logs, reproductions, plans, or reports.
- A ReviewerAgent inspects design, diffs, edge cases, regressions, and evidence.
- A SupervisorAgent accepts, rejects, delegates more work, or escalates.

This is an operating rule, not a prompt style.

## 6. State over Chat History

Conversation history is an input, not the source of truth.

The source of truth MUST be typed state:

- Agent Control Blocks
- task DAG
- blackboard entries
- event log
- artifacts
- evidence records
- locks
- approvals
- memory entries

The system MUST be able to recover meaningful task state without replaying an unstructured chat transcript as the only authority.

## 7. Storage Driver Principle

The kernel MUST define storage traits and schemas. It MUST NOT depend on a single database product as its identity.

SQLite is the default local driver.

PostgreSQL is the official production control-plane driver.

Object storage is the production artifact blob driver.

The kernel contract is the event and state schema, not the database engine.

## 8. Ecosystem Boundary Principle

External agent frameworks can participate as:

- distribution packages
- external tools
- model providers
- experimental workloads that use the current kernel contract

They MUST NOT be allowed to define:

- Agent Thread lifecycle
- syscall semantics
- evidence semantics
- permission semantics
- kernel state layout
- scheduling contract

Ecosystem integration should make Agent-OS useful without surrendering the
kernel contract or introducing alternate execution layers.

## 9. Production-First Principle

Agent-OS must be designed as production infrastructure from the first iteration.

Production-first does not mean cloud-first. It means:

- durable state
- typed schemas
- deterministic replay where possible
- crash recovery
- auditability
- permission boundaries
- conformance tests
- idempotent tool execution
- signed packages where needed
- conformance gates

Toy demos that cannot survive failure, review, or replay are not acceptable as architectural foundations.

