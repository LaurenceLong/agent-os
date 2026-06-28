# Provider System

Status: normative

Last updated: 2026-06-25

## 1. Purpose

Provider design in Agent-OS is system-level infrastructure, not an Agent Thread local detail.

Every Agent Thread that needs LLM cognition must obtain its stream from a unified Provider System. The Provider System is the single place that knows:

- which providers exist
- how models are named and aliased
- how credentials are resolved
- how routing policy works
- how fallback works
- how streaming sessions are opened
- how provider quirks are normalized
- how usage and cost are recorded

This is analogous to a `cc-switch` layer for Agent-OS: one unified provider configuration and routing surface for the whole system.

## 2. Core Principle

Agent Threads do not instantiate provider SDKs directly.

They request a stream-capable model session from the Provider System. The Provider System resolves configuration, capabilities, credentials, routing, transforms, and streaming semantics, then returns a normalized LLM stream.

The Agent Thread Runtime should know how to consume a normalized stream. It should not need to know Anthropic vs OpenAI vs Gemini vs local vLLM quirks.

## 3. Responsibilities

The Provider System owns:

- provider registry
- provider profiles
- model catalog
- model aliases
- credential resolution
- environment-specific overrides
- routing policy
- fallback policy
- rate limiting and quotas
- stream session lifecycle
- usage and cost accounting
- provider capability normalization
- provider-specific transform plugins
- audit and telemetry for model calls

## 4. Architecture

Logical components:

```text
Provider System
  -> Provider Registry
  -> Provider Profile Store
  -> Model Catalog
  -> Routing Policy Engine
  -> Credential Resolver
  -> Stream Session Manager
  -> Usage / Cost Meter
  -> Provider Adapter Layer
```

### 4.1 Provider Registry

Tracks available providers and adapter implementations.

### 4.2 Provider Profile Store

Stores unified provider configuration used by threads, environments, or distributions.

### 4.3 Model Catalog

Normalizes model identity, aliases, capabilities, limits, and provider-specific names.

### 4.4 Routing Policy Engine

Selects the effective provider and model for a request.

### 4.5 Credential Resolver

Resolves secrets from local config, environment, secret store, or worker scope.

### 4.6 Stream Session Manager

Creates normalized LLM stream sessions and owns their lifecycle.

### 4.7 Provider Adapter Layer

Implements provider-specific calls and transforms.

The adapter layer owns provider wire-format differences. The Agent Thread Runtime consumes provider-neutral tool calls with structured input objects; OpenAI-compatible adapters serialize those objects into `function.arguments` JSON strings at the API boundary, while Anthropic-compatible adapters pass the same objects as `tool_use.input`. Tool results are converted back through the adapter in the opposite direction before the next model turn. Runtime, Tool Broker, evidence, and replay records must not depend on provider-specific argument encoding.

The current implementation has live e2e coverage for both adapter styles. Those
tests run against real provider endpoints, generate the normal Agent-OS system
prompt, record provider request/response messages, and prove that the same
goal-driven tool scenarios reach 100% coverage through both OpenAI-compatible
and Anthropic-compatible calling.

## 5. Unified Configuration

Provider configuration must be unified and system-visible.

Suggested shape:

```yaml
provider_profiles:
  default-coding:
    routing_policy: coding-default
    default_provider: primary-llm-provider
    adapter: openai-compatible
    default_model_alias: coding-primary
    fallback_chain:
      - coding-primary
      - coding-fallback
    reasoning_defaults:
      effort: high
      summary: concise
    tool_visibility_profile: coding-tools
    timeout_ms: 120000
    max_output_tokens: 16000

routing_policies:
  coding-default:
    rules:
      - when:
          role: WorkerAgent
        use:
          model_alias: coding-primary
      - when:
          role: ReviewerAgent
        use:
          model_alias: review-primary

model_aliases:
  fast-read:
    provider: primary-llm-provider
    provider_model_name: vendor-fast-readable-model
  coding-primary:
    provider: primary-llm-provider
    provider_model_name: vendor-coding-primary-model
  review-primary:
    provider: secondary-llm-provider
    provider_model_name: vendor-review-primary-model
```

The model names above are placeholders. Real provider model names belong in the versioned Model Catalog so vendor naming changes do not make design docs stale. The exact file format can evolve, but the system-level idea is fixed: threads bind to profiles and policies, not to ad hoc provider SDK calls.

Minimal environment-based distributions may map `LLM_BASE_URL`, `LLM_MODEL`, `LLM_API_KEY`, and optional `LLM_API_STYLE` into provider profiles. Existing OpenAI-compatible variables can remain as compatibility aliases, but the normalized profile must still record the resolved adapter style for audit and replay.

## 6. Thread Integration

Each Agent Turn should resolve model execution through:

```text
Thread Config Snapshot
  -> provider_profile_id
  -> model_routing_policy_id
  -> requested_model_alias or explicit override
  -> reasoning profile
  -> environment selection
  -> task and role metadata
  -> Provider System
  -> Provider Stream Session
```

The runtime-facing call should conceptually look like:

```rust
let session = provider_system.open_stream_session(StreamRequest {
    thread_id,
    turn_id,
    provider_profile_id,
    model_routing_policy_id,
    requested_model_alias,
    role,
    task_id,
    reasoning_profile,
    tool_visibility_profile,
    output_schema,
})?;
```

## 7. Normalized Stream Contract

The Provider System returns a normalized stream event model.

Initial event families:

```text
StreamStarted
ReasoningStarted
ReasoningDelta
ReasoningCompleted
OutputTextStarted
OutputTextDelta
OutputTextCompleted
ToolCallProposed
ToolCallCompleted
UsageUpdated
ProviderWarning
ProviderRetry
ProviderFallback
StreamCompleted
StreamFailed
StreamCancelled
```

Agent Thread Runtime consumes these events and turns them into Agent Items and durable events.

## 8. Model Capability Catalog

Model selection should not be stringly typed.

The catalog should expose at least:

```yaml
model_alias: string
provider_id: string
provider_model_name: string
capabilities:
  streaming: boolean
  tool_calling: boolean
  reasoning: boolean
  image_input: boolean
  structured_output: boolean
limits:
  context_window: integer | null
  max_output_tokens: integer | null
cost:
  input_per_1m: number | null
  output_per_1m: number | null
```

This catalog is used by routing, tool visibility, and budgeting.

## 9. Override Rules

Overrides should follow a strict precedence order:

```text
hard policy
distribution policy
thread provider profile
turn override
role routing rule
model alias default
provider adapter default
```

Rules:

- a turn may request an override
- the Provider System decides whether the override is allowed
- a worker cannot jump to an arbitrary provider if policy forbids it
- provider selection is audited

## 10. Fallback and Failover

Fallback belongs to the Provider System, not each Agent Thread.

Fallback policy should support:

- retry same provider same model
- retry same provider different model
- switch provider same capability tier
- fail closed for strict tasks
- fail open for exploratory tasks when policy allows

Every fallback must emit a durable event.

## 11. Credentials and Secrets

Credential resolution belongs to the Provider System.

Sources may include:

- local development config
- environment variables
- worker-scoped secret mount
- secret store
- distribution deployment config

Threads must never see raw provider secrets unless explicitly required.

## 12. Relationship to Model Gateway

`Model Gateway` should be treated as the runtime-facing stream facade inside the broader Provider System.

That means:

- Provider System is the system-level module
- Model Gateway is the narrow execution facade that Agent Thread Runtime calls

This preserves a clean runtime API without collapsing provider design into thread-local code.

## 13. Package Boundary

Recommended packages:

```text
crates/
  agent-os-provider/            # system-level provider control plane
  agent-os-provider-adapters/   # provider-specific adapters
  agent-os-model-catalog/       # normalized models and capabilities
```

Optional later packages:

```text
crates/
  agent-os-provider-secrets/
  agent-os-provider-metering/
```

## 14. Conformance Tests

Minimum tests:

1. Agent Thread cannot access provider SDK directly.
2. Stream session must be opened through Provider System.
3. Same profile and routing policy resolve deterministically.
4. Forbidden provider override is rejected.
5. Fallback emits durable event.
6. Usage and cost are recorded even across fallback.
7. Provider capability mismatch is rejected before stream open.
8. Turn-scoped provider session state does not leak across turns.
9. Secret values are not exposed in thread-visible state.
10. Model alias changes do not require Agent Thread code changes.
