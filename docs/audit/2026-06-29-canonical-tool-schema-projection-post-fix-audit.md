# Canonical Tool Schema Projection Post-Fix Audit

Date: 2026-06-29

## Repository State

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree at pre-fix audit: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Pre-fix audit:
  `docs/audit/2026-06-29-canonical-tool-schema-projection-pre-fix-audit.md`
- Worktree status after the fix remains dirty. Existing unrelated modified and
  untracked v0.2 files were preserved.

## Implemented Fixes

1. Added descriptor projection to `ModelContextProjection` so each model turn
   receives sorted kernel `ToolDescriptor` records from the current state.
2. Replaced adapter-local core tool JSON in the OpenAI-compatible adapter with
   projection from `ToolDescriptor.description` and
   `ToolDescriptor.model_input_schema`.
3. Kept Anthropic-compatible tools as a pure conversion from the same
   OpenAI-compatible descriptor projection.
4. Moved parser runtime field injection to
   `ToolRuntimeInputPolicy.injected_fields`; `workspace_root` and `cwd` are no
   longer hard-coded by tool name.
5. Kept descriptor risk levels as the default model-action risk source, while
   preserving action-specific `agent_control` risk classification.
6. Added model-facing metadata for all core built-in tool descriptors through
   focused kernel profile-seed schema helpers.
7. Split filesystem and shell descriptor seeding into
   `crates/agent-os-kernel/src/profile_seed/tools/filesystem.rs`, bringing
   `crates/agent-os-kernel/src/profile_seed/tools.rs` down to 571 lines.
8. Added a regression test proving that changing a kernel descriptor changes
   both OpenAI-compatible and Anthropic-compatible model tool schemas.
9. Updated contract docs to require core and dynamic model-visible schemas to
   project from registered kernel descriptors.

## Changed Files In This Fix

- `crates/agent-os-kernel/src/profile_seed.rs`
- `crates/agent-os-kernel/src/profile_seed/tools.rs`
- `crates/agent-os-kernel/src/profile_seed/tool_schemas.rs`
- `crates/agent-os-kernel/src/profile_seed/tools/filesystem.rs`
- `crates/agent-os-thread/src/model.rs`
- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/ecosystem_projection.rs`
- `crates/agent-os-thread/src/openai/tools.rs`
- `crates/agent-os-thread/src/openai/parser.rs`
- `crates/agent-os-thread/src/openai/tests/support.rs`
- `crates/agent-os-thread/src/openai/tests/unit.rs`
- `docs/10-kernel-design/permission-tool-evidence-model.md`
- `docs/20-implementation/conformance-and-quality.md`
- `docs/audit/2026-06-29-canonical-tool-schema-projection-pre-fix-audit.md`
- `docs/audit/2026-06-29-canonical-tool-schema-projection-post-fix-audit.md`
- `docs/superpowers/specs/2026-06-29-canonical-tool-schema-projection-design.md`
- `docs/superpowers/plans/2026-06-29-canonical-tool-schema-projection.md`

## Validation Results

Fresh validation after implementation:

```text
cargo fmt --all
cargo test -p agent-os-thread openai::tests::unit -- --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --message-format short
```

Observed results:

- `cargo fmt --all`: passed.
- `cargo test -p agent-os-thread openai::tests::unit -- --nocapture`:
  21 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --message-format short`: 137 passed, 0 failed,
  10 ignored live LLM tests.

## Forward-Only Notes

- No adapter-local static core tool schema fallback remains in
  `agent-os-thread/src/openai/tools.rs`.
- Descriptors without model-facing description or `model_input_schema` are not
  projected into provider tool views.
- Runtime-only fields remain kernel runtime input fields and are injected from
  descriptor policy, not supplied by the model.
- Dynamic MCP visibility is authorized through descriptor driver class and
  required resource scopes.

## Remaining Gaps

- Remote MCP, OAuth MCP, plugin runtimes, and JavaScript tool drivers remain
  future ADR work.
- Ignored live LLM e2e coverage for the combined imported rules, skill, and MCP
  path still depends on provider credentials and spend policy.

