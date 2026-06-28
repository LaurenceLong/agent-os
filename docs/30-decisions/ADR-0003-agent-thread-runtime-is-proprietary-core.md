# ADR-0003: Agent Thread Runtime is proprietary core infrastructure

Status: accepted

Date: 2026-06-25

## Context

Agent-OS requires an execution unit that behaves like a kernel-managed thread:

- has an Agent Control Block
- obeys lifecycle state transitions
- yields at cooperative scheduling boundaries
- uses capability-checked syscalls
- receives scoped context
- submits artifacts
- attaches evidence
- obeys role restrictions
- supports recovery and audit

Open-source agent frameworks do not provide this exact execution contract.

## Decision

Agent-OS will implement its own Agent Thread Runtime.

Open-source agents may be supported as guest runtimes or compatibility layers only if all state changes, tool calls, artifacts, and evidence flow through Agent-OS kernel contracts.

## Consequences

Positive:

- runtime semantics match kernel needs
- permissions and evidence can be enforced below prompts
- long-task recovery can be designed into the core
- distributions share a stable execution unit

Negative:

- more implementation work
- fewer immediate integrations
- must maintain LLM/provider adapters directly or through controlled drivers

## Implications

The first production runtime MUST implement Agent Thread lifecycle, checkpointing, tool/syscall mediation, artifact/evidence handling, and final submission gates as proprietary core infrastructure.

The v0.1 core role set is narrowed by ADR-0009 to SupervisorAgent, WorkerAgent, and ReviewerAgent. Distribution workflow step labels MAY be provided through workflow prompts, examples, and policy packs, but they are not kernel-required roles.
