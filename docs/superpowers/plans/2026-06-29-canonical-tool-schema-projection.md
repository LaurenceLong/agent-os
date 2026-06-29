# Canonical Tool Schema Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Project model-visible tool schemas from kernel `ToolDescriptor` records for core built-in tools.

**Architecture:** Kernel descriptors become the single source for core tool descriptions, model input schemas, runtime injection policy, driver class, and risk metadata. Agent Thread runtime includes sorted descriptors in each model request, and provider adapters only convert neutral descriptors into OpenAI-compatible or Anthropic-compatible shapes.

**Tech Stack:** Rust workspace, serde/serde_json schemas, existing kernel event/state projection, existing OpenAI/Anthropic-compatible adapter tests.

## Global Constraints

- No backward compatibility, fallbacks, legacy adapters, feature flags, or migration shims.
- `agent-os-sys` owns shared ABI/data types and stays dependency-light.
- `agent-os-kernel` owns authoritative tool metadata and authorization.
- `agent-os-thread` converts model actions into kernel calls and must not duplicate kernel authority.
- Production Rust modules should stay focused and preferably under 400 lines; files over 600 lines must be split before substantial new behavior.
- All production code changes follow red/green TDD.
- Run relevant tests and `cargo clippy --workspace --all-targets -- -D warnings` before handoff when feasible.

---

### Task 1: Descriptor Projection Failing Tests

**Files:**
- Modify: `crates/agent-os-thread/src/openai/tests/unit.rs`
- Modify: `crates/agent-os-thread/src/openai/tests/support.rs`

**Interfaces:**
- Consumes: `Kernel::register_tool_descriptor`, `Kernel::state_snapshot`, `ModelTurnRequest`
- Produces: tests requiring `ModelContextProjection.tool_descriptors`,
  descriptor-driven tool projection, and descriptor runtime field injection

- [ ] **Step 1: Write failing descriptor projection test**

Add a test that registers a modified `read_file` descriptor with a unique
description and model schema, refreshes request context descriptors from kernel
state, and asserts both OpenAI and Anthropic projections use the descriptor.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-os-thread request_tool_view_projects_core_tools_from_kernel_descriptors -- --nocapture`

Expected: compile failure or assertion failure because `ModelContextProjection`
does not yet carry core descriptors and OpenAI core schemas are static.

### Task 2: Runtime Tool Descriptor Projection

**Files:**
- Modify: `crates/agent-os-thread/src/model.rs`
- Modify: `crates/agent-os-thread/src/runtime/ecosystem_projection.rs`
- Modify: `crates/agent-os-thread/src/runtime.rs`
- Modify: `crates/agent-os-thread/src/openai/tests/support.rs`

**Interfaces:**
- Produces: `ModelContextProjection.tool_descriptors: Vec<ToolDescriptor>`

- [ ] **Step 1: Add `tool_descriptors` to model context**

Project sorted `KernelState.tool_descriptors` by tool name into every model
turn request.

- [ ] **Step 2: Run focused test**

Run: `cargo test -p agent-os-thread request_tool_view_projects_core_tools_from_kernel_descriptors -- --nocapture`

Expected: still failing because adapter projection remains static.

### Task 3: Descriptor-Driven Provider Tool Projection

**Files:**
- Modify: `crates/agent-os-thread/src/openai/tools.rs`
- Modify: `crates/agent-os-thread/src/openai/parser.rs`
- Modify: `crates/agent-os-thread/src/openai/tests/unit.rs`

**Interfaces:**
- Consumes: `ToolDescriptor.model_input_schema`, `ToolDescriptor.description`,
  `ToolDescriptor.runtime_input_policy`, `ToolDescriptor.risk_level`
- Produces: OpenAI and Anthropic tool schemas from descriptors

- [ ] **Step 1: Replace static core projection**

Build OpenAI function tool JSON from descriptors in `ModelTurnRequest.context`.
Use `model_input_schema` when present and fail closed by omitting descriptors
without model-facing schema.

- [ ] **Step 2: Keep authority redaction**

Filter by permission set, S-level control-plane gates, and privileged
`agent_control` action redaction after descriptor projection.

- [ ] **Step 3: Move parser injection to descriptor policy**

Use `runtime_input_policy.injected_fields` to inject `workspace_root` and `cwd`.
Use descriptor risk level except for action-specific `agent_control` risk.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p agent-os-thread openai::tests::unit -- --nocapture`

Expected: descriptor projection tests pass after kernel descriptors are
enriched in Task 4.

### Task 4: Split And Enrich Core Tool Descriptors

**Files:**
- Modify: `crates/agent-os-kernel/src/profile_seed/tools.rs`
- Create: `crates/agent-os-kernel/src/profile_seed/tool_schemas.rs`

**Interfaces:**
- Produces: enriched core `ToolDescriptor` records with model descriptions,
  model input schemas, and runtime injection policy

- [ ] **Step 1: Extract shared schema builders**

Move repeated model schema builders, including `permission_set_schema`, into a
focused module so `tools.rs` does not grow further.

- [ ] **Step 2: Enrich core descriptors**

For each model-visible core tool, set `description`, `model_input_schema`, and
runtime injected fields where the model must not provide runtime-only values.

- [ ] **Step 3: Run focused tests**

Run: `cargo test -p agent-os-thread openai::tests::unit -- --nocapture`

Expected: all OpenAI unit tests pass.

### Task 5: Contract Docs And Validation

**Files:**
- Modify: `docs/10-kernel-design/permission-tool-evidence-model.md`
- Modify: `docs/20-implementation/conformance-and-quality.md`
- Create: `docs/audit/2026-06-29-canonical-tool-schema-projection-post-fix-audit.md`

**Interfaces:**
- Produces: updated current-contract docs and post-fix audit record

- [ ] **Step 1: Update docs**

Record that core and dynamic tools project model schemas from registered
`ToolDescriptor` records.

- [ ] **Step 2: Run verification**

Run:
`cargo fmt --all`
`cargo test -p agent-os-thread openai::tests::unit -- --nocapture`
`cargo test --workspace --message-format short`
`cargo clippy --workspace --all-targets -- -D warnings`

- [ ] **Step 3: Write post-fix audit**

Record changed files, validation output, forward-only choices, and remaining
gaps.

