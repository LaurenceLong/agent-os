# Tool Lifecycle Governance Plan

Status: planning

Last updated: 2026-07-03

## Goal

Make every model-visible and runtime-only tool pass through a single governed
lifecycle contract: discovery, projection, validation, approval, execution,
progress, result shaping, evidence, telemetry, cancellation, and replay.

## Non-Goals

- Do not let runtime crates mutate tool state directly.
- Do not add ad hoc hook systems that bypass kernel policy.
- Do not preserve legacy tool names or JSON shapes when clearer contracts are
  available.

## Current Agent-OS State

The kernel already owns tool descriptors, registration, permission checks,
validation, invocation events, foreground timeout behavior, managed output, and
evidence attachment. `ToolCallStatus` already models proposed, running,
completed, failed, denied, cancelled, and timed-out states. `ToolPlan`,
`ToolExposure`, and `ToolPlanningMode` now define the per-turn model-visible
surface, including direct, deferred, hidden, and disabled entries; created plans
are emitted as replayable kernel events.

The gap is governance depth:

- descriptor lifecycle policy is declarative but not fully enforced across every
  execution branch;
- direct/deferred/hidden/disabled model exposure is a first-class plan, but
  richer per-entry cancellation, parallelism, and output-shaping policy still
  needs to move into that plan;
- tool search is governed for deferred MCP tools, but install-candidate and
  plugin suggestion flows are still future work;
- lifecycle hooks/contributors are not represented as kernel resources;
- parallelism and runtime cancellation are not explicit per tool;
- model-visible output shaping is not a separate post-execution policy.

## Codex Reference

Codex separates executable registry, per-turn tool spec planning, deferred tool
search, runtime dispatch, pre/post tool use hooks, lifecycle notifications, and
dispatch traces. This gives Codex a dynamic model-visible tool surface without
making the model or runtime the source of authority.

## Target Agent-OS Contract

Agent-OS should introduce a kernel-owned `ToolPlan` projection for each runtime
turn:

- direct model-visible tools;
- deferred discoverable tools;
- runtime-only tools;
- hidden kernel/internal tools;
- disabled or denied tools with reasons;
- parallelism and cancellation properties;
- required approval and permission profile effects;
- output presentation and managed-output policy.

Tool execution should follow one canonical event order:

```text
planned -> proposed -> validated -> pending_approval? -> started
-> progressed* -> completed|failed|denied|cancelled|timed_out
```

Pre-execution and post-execution policy should be expressed as typed kernel
rules. Package/plugin contributors may register rules later, but the kernel must
emit and reduce the authoritative events.

## Crate Ownership

- `agent-os-sys`: `ToolPlan`, `ToolExposure`, lifecycle rule, cancellation, and
  output-shaping data types.
- `agent-os-kernel`: registry, planning, permission mediation, rule execution,
  lifecycle events, evidence, replay.
- `agent-os-thread`: asks the kernel for the turn tool plan and projects it into
  provider-specific model schemas.
- `agent-os-conformance`: public tool lifecycle and projection contract tests.

## Implementation Slices

1. Add `ToolExposure` and `ToolPlan` data types. Implemented.
2. Make runtime tool projection come from a kernel `plan_tools_for_turn` API.
   Implemented.
3. Move direct/deferred/hidden/disabled filtering into kernel-owned policy.
   Implemented for core, MCP, image-capability, permission, security-level, and
   planning-mode decisions.
4. Add a model-visible `tool_search` only when deferred tools exist.
   Implemented for deferred MCP tool descriptors.
5. Enforce descriptor lifecycle policy for foreground/background, cancellation,
   managed output, and orphan recovery.
6. Add typed pre-execution and post-execution policy rules.
7. Add dispatch trace records and lifecycle audit events. Tool plans are now
   replayable via `ToolPlanCreated`; richer per-call dispatch traces remain.
8. Update conformance to assert exact model-visible tool surfaces by profile,
   provider capability, and S-level.

## Validation

- Unit tests for planning decisions, exposure filtering, lifecycle event order,
  approval branches, denied branches, timeout branches, and post-result shaping.
- Integration tests for kernel plus runtime projection across OpenAI-compatible
  and Anthropic-compatible adapters.
- Conformance tests that assert model-visible tools for normal, restricted, and
  delegated threads.
- Ignored live LLM e2e tests for deferred tool discovery and final submission.

## Forward-Only Notes

Existing descriptor fields that duplicate the new lifecycle plan should be
removed or folded into the canonical policy shape. Agent-OS should have one tool
planning path.
