# ADR-0004: Agent Thread Core Module is native core infrastructure

Status: accepted

Date: 2026-06-25

## Context

Agent-OS needs a production Agent Thread runtime. That runtime must be owned by
the Agent-OS kernel contract instead of delegated to any external agent
framework.

The project must keep its implementation authority inside Agent-OS documents,
typed syscalls, replayable events, permissions, evidence, artifacts, provider
contracts, and conformance tests.

## Decision

Agent-OS will implement a dedicated Agent Thread Core Module with:

- Agent Thread, Agent Turn, Agent Step, Agent Item, Tool Invocation, and Evidence Record as first-class objects.
- Agent Thread Control Block as kernel-owned state.
- Submission Queue and Event Queue semantics.
- Turn-scoped model sessions.
- system-level Provider System with a runtime-facing Model Gateway facade.
- Tool Broker syscall pipeline.
- deny-first capability and permission checks.
- lifecycle policy hooks that may restrict but cannot grant authority.
- Agent Control and Agent Registry for spawn, capacity, hierarchy, residency, and inter-agent communication.
- workspace, process, permission, context, and memory isolation.
- event-first recovery and conformance tests.

External agents may be hosted as guest runtimes only after they comply with
Agent-OS syscalls, permissions, artifact, evidence, and audit contracts.

## Consequences

Positive:

- Agent-OS has a stable kernel execution unit.
- Development can start from a precise module contract.
- Product maturity is judged against Agent-OS contracts rather than borrowed architecture.
- The design remains native to Agent-OS.
- Third-party distributions can target a stable ABI.

Negative:

- More initial engineering work than wrapping an existing framework.
- Agent-OS must own model-provider adaptation, tool lifecycle, and replay semantics.
- Conformance tests are mandatory early, not optional polish.

## Required Follow-Up

Implementation MUST start with the Agent Thread protocol skeleton before building UI, marketplace, or distributed deployment.

The first implementation milestone MUST prove replayable lifecycle state without using any LLM.
