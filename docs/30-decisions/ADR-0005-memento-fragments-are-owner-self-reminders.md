# ADR-0005: Memento Fragments are owner self-reminders

Status: accepted

Date: 2026-06-25

## Context

Agent-OS needs a mechanism for an Agent Thread to remind its future self what to do after a discontinuity.

Discontinuities include:

- spawning a child Agent Thread
- waiting for child completion
- waiting for approval
- waiting for a long-running tool
- context compaction
- suspension and resume
- review and verification callbacks

This mechanism was initially described as a "paper note", but it should not mean parent-child note passing. The correct metaphor is closer to *Memento*: external memory fragments that preserve future intent when continuous internal memory is unreliable.

## Decision

Agent-OS will introduce Memento Fragments.

A Memento Fragment is an immutable self-reminder owned by the Agent Thread that writes it. It may be anchored to child completion or another future event. The event can trigger projection of the reminder back to the owner, but the child Agent Thread cannot read or mutate the fragment.

Child-facing instructions remain separate as assignment payloads.

## Consequences

Positive:

- parent Agent Threads can delegate work without losing their own continuation plan
- child Agent Threads cannot corrupt parent intent
- resume and callback behavior becomes structured and replayable
- compaction can preserve future intent without dumping full history
- UI can display reminders as external memory shards without pretending they are facts

Negative:

- the kernel must manage another lifecycle entity
- spawn and callback paths must distinguish child assignment from owner reminder
- implementation must enforce strict visibility and immutability

## Required Follow-Up

The Agent Thread protocol skeleton MUST include Memento Fragment events.

Conformance tests MUST prove that child Agent Threads cannot read or mutate parent Memento Fragments, and that owner Agent Threads must supersede rather than edit armed fragments.

