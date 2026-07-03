# ADR-0006: Agent Thread communication is capability-scoped

Status: accepted

Date: 2026-06-25

## Context

Agent Threads need to communicate. Workers may need to report blockers to Supervisor, publish findings to blackboard channels, or ask humans for clarification.

Unrestricted communication would create noise, prompt-injection surfaces, human-attention waste, and authority escalation. A worker should not automatically gain the right to broadcast globally or contact a human simply because it was spawned.

## Decision

Agent-OS will make communication a kernel-mediated capability.

At Agent Thread creation time, the creator assigns a communication profile. The profile defines:

- whether the worker can send messages to Supervisor
- whether the worker can post to blackboard
- whether blackboard posts are task, goal, workspace, or global scoped
- whether blackboard posts can broadcast to subscribers
- whether the worker can contact a human
- which message types are allowed
- which routes can trigger receiver turns
- which routes require review before delivery

The worker cannot widen its own communication profile.

## Consequences

Positive:

- worker communication is useful but bounded
- human attention becomes a governed resource
- blackboard broadcast remains typed and auditable
- Supervisor can delegate without giving every worker global voice
- producer-to-producer coordination remains Supervisor-routed instead of becoming a chat mesh
- communication decisions become replayable and testable

Negative:

- spawn configuration is more complex
- communication needs schemas, delivery state, and conformance tests
- overly narrow profiles may block useful reports unless role defaults are well designed

## Required Follow-Up

The Agent Thread protocol skeleton MUST include communication profile, message envelope, delivery events, and blackboard channel posting.

Conformance tests MUST prove that workers cannot send to Supervisor, blackboard, or human routes unless their creation-time profile allows it. ADR-0009 narrows v0.1 core communication to Supervisor-routed coordination plus scoped blackboard and human routes.
