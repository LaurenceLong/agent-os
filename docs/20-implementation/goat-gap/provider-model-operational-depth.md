# Provider And Model Operational Depth Plan

Status: planning

Last updated: 2026-07-03

## Goal

Make model operations explicit, observable, and provider-neutral: capability
resolution, context limits, streaming, retries, rate limits, auth/account state,
model catalog refresh, request transforms, structured output, reasoning options,
and error classification should all have typed Agent-OS contracts.

## Non-Goals

- Do not bury provider behavior in CLI flags or raw JSON option maps.
- Do not let provider adapters bypass kernel-visible runtime state.
- Do not add provider-specific compatibility branches when a typed transform
  policy is the clearer contract.

## Current Agent-OS State

Agent-OS already has provider profiles, model identifiers, endpoint styles,
model limits, model capabilities, retry/transform/reasoning defaults, and
classified OpenAI-compatible API errors. Runtime adapters can target
OpenAI-compatible chat completions, OpenAI responses, and Anthropic messages.
Kernel provider stream sessions also persist typed stream events, usage, retry
and warning events, and app `provider/usage/read` projects a bounded operation
timeline with omitted-event accounting.

The gap is operational depth:

- runtime calls are non-streaming;
- model catalog resolution is mostly static config and presets;
- provider account/auth/rate-limit state is not a first-class runtime resource;
- retry and backoff behavior is not deeply integrated with provider telemetry;
- context-limit enforcement and overflow recovery are not yet a complete runtime
  loop policy;
- provider operation timelines are projected from existing kernel events, but
  live adapter streaming still needs to feed detailed text, reasoning, tool-call,
  refusal, usage, error, and completion chunks into that timeline.

## Codex Reference

Codex separates provider identity/capabilities/auth/account/error mapping from
model catalog management and client transport. Its client stack includes
request construction, retry, SSE streaming, telemetry, and provider API/rate
limit handling. Model managers combine static presets, configured providers,
remote model data, and cache state.

## Target Agent-OS Contract

Agent-OS should define:

- `ProviderRuntime`: provider identity, endpoint style, auth scope, account
  state, capability bounds, transport policy, and health state.
- `ModelCatalog`: resolved models, limits, capabilities, cache timestamp,
  provenance, and refresh policy.
- `ModelRoute`: selected provider/model plus reason, capability constraints,
  context/output limits, tool visibility effects, and request transform policy.
- `ProviderOperation`: request id, streaming state, retry attempts, rate-limit
  observations, usage, classified errors, and audit artifact references.
- `ProviderStreamEvent`: typed chunks for text, reasoning, tool calls, refusal,
  usage, error, and completion.

The kernel should own runtime policy and durable operation state. The thread
crate should own adapter implementation and stream parsing.

## Crate Ownership

- `agent-os-sys`: provider runtime, model catalog, route, operation, stream
  event, error, and usage data types.
- `agent-os-config`: cross-platform config loading and model catalog file
  parsing.
- `agent-os-kernel`: route admission, budget/permission effects, operation
  recording, usage/rate-limit projection, replay.
- `agent-os-thread`: provider clients, request builders, stream parsers,
  retry/backoff mechanics, model action parsing.
- `agent-os-conformance`: provider routing and runtime behavior tests.

## Implementation Slices

1. Normalize provider profile and model catalog into one current contract.
2. Add provider operation events and usage/rate-limit projections. Usage,
   retry, warning, and bounded operation timeline projection are implemented;
   richer rate-limit state remains.
3. Add streaming response parsing for the OpenAI-compatible path.
4. Make tool-call streaming feed the existing runtime parser without provider
   leakage.
5. Add typed retry/backoff policy with classified errors.
6. Add context-limit enforcement before request submission.
7. Add model catalog refresh/cache where remote refresh failure records an
   explicit stale-or-unavailable state; configured local catalogs remain usable
   only when they are the selected source of truth.
8. Extend Anthropic-compatible streaming after OpenAI-compatible streaming is
   stable.

## Validation

- Unit tests for request builders, transform policy, catalog resolution,
  context-limit checks, error classification, retry/backoff, and stream parsing.
- Integration tests with a fake HTTP server for streaming, rate-limit headers,
  transient errors, context overflow, auth failures, and malformed chunks.
- Runtime conformance tests proving provider-neutral tool-call handling.
- Ignored live LLM e2e tests for OpenAI-compatible and Anthropic-compatible
  streaming through the normal Agent Thread runtime loop.

## Forward-Only Notes

Raw provider option passthrough should narrow over time. Options that affect
Agent-OS behavior, such as reasoning, max output, tool mode, streaming, and
structured output, should become typed policy.
