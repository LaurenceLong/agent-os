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
- how retry and failure policy works
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
- retry policy and fail-closed failure handling
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

Current seed shape:

```yaml
provider_profiles:
  prov_default:
    routing_policy_id: route_default
    default_provider_id: primary-provider
    default_model_alias: general-primary
    allowed_model_aliases:
      - coding-primary
      - review-primary
      - general-primary
      - text-only
    credential_ref:
      credential_ref_id: cred_default_llm
      source: environment
      name: AGENT_OS_LLM_API_KEY
    retry_policy:
      max_attempts: 2
      backoff_ms: 0
    transform_policy:
      adapter_style: openai-compatible
    reasoning_defaults: {}
    tool_visibility_profile: null
    timeout_ms: 120000
    max_output_tokens: 16000

routing_policies:
  route_default:
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
  coding-primary:
    provider_id: primary-provider
    provider_model_name: primary-coding-model
  review-primary:
    provider_id: primary-provider
    provider_model_name: primary-review-model
  general-primary:
    provider_id: primary-provider
    provider_model_name: primary-general-model
  text-only:
    provider_id: primary-provider
    provider_model_name: primary-text-model
```

The active `model_aliases` map is the current Model Catalog contract. A provider route resolves through the thread's provider profile, routing policy, allowed alias list, and active alias record; missing, inactive, or capability-incompatible aliases are rejected before a stream session opens. Threads bind to profile IDs, credential references, retry policy, transform policy, and routing policies, not to ad hoc provider SDK calls.

Minimal environment-based distributions may map `LLM_BASE_URL`, `LLM_MODEL`, `LLM_API_KEY`, and optional `LLM_API_STYLE` into provider profiles. `LLM_*` is the canonical environment surface. Other provider-specific environment names are outside the core contract.

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

## 10. Retry and Failure

Retry belongs to the Provider System, not each Agent Thread.

Retry policy supports:

- retrying the selected provider and model according to the active provider
  profile
- recording each retry as a durable provider stream event
- failing closed after retry policy is exhausted
- releasing the provider-slot lease on completion, failure, or cancellation

Routing chooses the provider and model before stream open. Once a stream session
is opened, the provider and model remain fixed for that session. If the selected
route cannot produce a valid stream, the stream fails and the failure is
recorded.

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
5. Provider retry emits a durable event.
6. Failed streams release provider-slot leases and record failure state.
7. Usage and cost are recorded for metered provider sessions.
8. Provider capability mismatch is rejected before stream open.
9. Turn-scoped provider session state does not leak across turns.
10. Secret values are not exposed in thread-visible state.
11. Model alias changes do not require Agent Thread code changes.
