# Canonical Tool Schema Projection Pre-Fix Audit

Date: 2026-06-29

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status at audit start: dirty with existing v0.2 goal-control,
  S-level permission, provider-config, and ecosystem-ingestion changes.
- Current validation before this audit:
  - `cargo check --workspace --message-format short`: passed
  - `cargo test --workspace --message-format short`: passed, 136 passed,
    10 ignored live LLM tests

## Audit Scope

This audit covers the remaining current-contract gap recorded in
`docs/audit/2026-06-29-ecosystem-ingestion-post-fix-audit.md`: dynamic model
tool projection has started for imported MCP tools, but core built-in OpenAI
tool schemas still live as static provider-adapter JSON.

The scope is limited to canonical model-tool schema projection from kernel
`ToolDescriptor` records into provider adapter formats. Remote MCP, OAuth MCP,
plugin runtimes, JavaScript tool drivers, and new live LLM e2e coverage remain
outside this fix.

## Current-Contract Gaps

1. `crates/agent-os-thread/src/openai/tools.rs` defines core built-in tool
   descriptions and model input schemas directly in the OpenAI adapter.
2. Kernel `ToolDescriptor` is already the authoritative registry for tool
   runtime schemas and authorization metadata, but most core descriptors lack
   model-facing descriptions and schemas.
3. OpenAI and Anthropic request builders can drift from the kernel registry
   because they still project core tools from adapter-local JSON.
4. `crates/agent-os-kernel/src/profile_seed/tools.rs` is over 600 lines and
   should be split before adding substantial new schema behavior.

## Future-Roadmap Gaps

- Provider-specific presentation tuning beyond OpenAI-compatible and
  Anthropic-compatible shape conversion is not part of this audit.
- Dynamic registration from every future third-party tool driver remains future
  ADR work.
- Live LLM e2e for combined imported rules, skills, and MCP remains ignored
  until provider credentials and spend policy are available.

## Intended Fix Scope

1. Add focused tests proving core model tool definitions come from kernel
   descriptors instead of adapter-local static JSON.
2. Add a model-tool projection to `ModelContextProjection` so provider adapters
   receive the current kernel tool registry for each turn.
3. Populate model-facing `description`, `model_input_schema`, and
   `runtime_input_policy` for core built-in tools in the kernel profile seed.
4. Split core tool descriptor seeding into focused modules before growing the
   schema code further.
5. Keep OpenAI and Anthropic adapters as format adapters over the same
   descriptor projection.

## Validation Plan

- Red/green focused tests for descriptor-driven OpenAI/Anthropic core tool
  projection.
- `cargo fmt --all`
- `cargo test -p agent-os-thread openai::tests::unit -- --nocapture`
- `cargo test --workspace --message-format short`
- `cargo clippy --workspace --all-targets -- -D warnings`

