# ADR-0001: Agent-OS is a microkernel-style runtime

Status: accepted

Date: 2026-06-25

## Context

Agent-OS aims to make agents observable, schedulable, recoverable, auditable, governable, and composable.

Existing agent frameworks provide useful orchestration, memory, tool, or coding-agent capabilities, but they do not define the full operating model required by Agent-OS:

- Agent Control Block
- Agent Thread lifecycle
- capability-checked syscalls
- evidence-first final answer
- producer-reviewer-verifier separation
- typed blackboard
- durable replay
- distribution conformance

## Decision

Agent-OS will be designed as a microkernel-style runtime.

The kernel owns the minimal set of durable governance semantics. Domain behavior, prompts, tools, UI, and deployment choices live in distributions or drivers.

## Consequences

Positive:

- stable kernel semantics
- third-party distribution model
- clearer security boundary
- easier conformance testing
- less framework lock-in

Negative:

- slower first demo
- more upfront schema and ABI work
- more responsibility for runtime correctness

## Implications

Implementation MUST prioritize kernel contracts before UI or domain automation features.

