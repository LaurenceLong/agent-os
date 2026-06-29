# Canonical Tool Schema Projection Design

Date: 2026-06-29

## Goal

Make kernel `ToolDescriptor` records the canonical source for model-visible
tool descriptions and input schemas for core built-in tools, matching the
existing dynamic MCP projection path.

## Architecture

The kernel continues to own tool authority, runtime schemas, risk levels,
driver classes, and model-facing metadata. Agent Thread runtime projects the
current tool registry into each `ModelTurnRequest`. Provider adapters convert
that neutral projection into OpenAI-compatible or Anthropic-compatible tool
format without redefining core tool bodies.

Core built-in tool descriptors gain explicit `description`,
`model_input_schema`, and `runtime_input_policy` values. Runtime-only fields
such as `workspace_root` and `cwd` remain kernel input-schema fields, while
model schemas expose only the arguments the model may provide. Parser-side
input injection uses descriptor policy rather than hard-coded per-tool field
injection.

## Components

- `agent-os-sys`: existing `ToolDescriptor` and `ToolRuntimeInputPolicy`
  remain the shared ABI. No compatibility fields are added.
- `agent-os-kernel`: profile-seeded core tools are split by family and enriched
  with model-facing metadata.
- `agent-os-thread`: `ModelContextProjection` carries sorted
  `ToolDescriptor` records. OpenAI and Anthropic adapters project the same
  descriptor list into provider-specific shapes.
- Tests: OpenAI unit tests prove descriptor-driven core projection,
  Anthropic mirroring, permission redaction, control-plane redaction, and
  parser runtime-field injection.

## Data Flow

1. `Kernel::new()` seeds core `ToolDescriptor` records.
2. Runtime snapshots kernel state at each model turn.
3. Runtime copies sorted tool descriptors into `ModelContextProjection`.
4. Provider adapter filters descriptors by permission set and security level.
5. Provider adapter converts allowed descriptors into provider tool schema.
6. Parser maps model function calls back through descriptor policy and risk
   metadata before kernel invocation.

## Error Handling

Descriptors missing model-facing metadata are rejected from provider projection
for model-visible tool views rather than silently falling back to old static
JSON. Permission and S-level redaction remain fail-closed: hidden tools are not
projected, and kernel authorization still rechecks every invocation.

## Testing

The first test asserts that changing a kernel descriptor changes the projected
OpenAI and Anthropic tool schema. A second test asserts descriptor policy
injects runtime-only fields into parsed tool input. Existing permission and
agent-control redaction tests stay in place to prevent broadening authority.

## Out Of Scope

- Remote MCP, OAuth MCP, plugin runtime, and JavaScript tool execution.
- Live LLM e2e spend.
- Provider-specific schema dialect features beyond current OpenAI and
  Anthropic shapes.

