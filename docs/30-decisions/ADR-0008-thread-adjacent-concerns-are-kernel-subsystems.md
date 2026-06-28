# ADR-0008: Thread-adjacent concerns are kernel subsystems

Status: accepted

Date: 2026-06-25

## Context

As Agent-OS design grows, there is a constant temptation to keep adding more responsibility directly into Agent Thread Runtime:

- role semantics
- permission ceilings
- sandbox and execution setup
- scheduling priority
- budget handling
- resource conflict handling

That path would turn Agent Thread into a monolith and make distributions hard to certify.

## Decision

Agent-OS will lift the following concerns into dedicated system-level subsystems:

- Role and Profile System
- Execution Environment System
- Scheduler and Resource Arbitration

Agent Thread Runtime will consume resolved bindings, leases, and scheduler decisions from the kernel rather than owning these policies locally.

## Consequences

Positive:

- thread runtime stays smaller and more testable
- distributions can extend roles and environments without redefining thread semantics
- scheduling, isolation, and budget behavior become auditable
- conformance suites can target kernel contracts instead of prompt conventions

Negative:

- more early control-plane design work is required
- bootstrap implementation needs profile, environment, and scheduler stores sooner
- local development needs a clean default configuration story

## Required Follow-Up

Normative kernel design documentation MUST include:

- Role and Profile System
- Execution Environment System
- Scheduler and Resource Arbitration

The roadmap and conformance documents MUST treat these as first-class implementation milestones rather than optional future refactors.
