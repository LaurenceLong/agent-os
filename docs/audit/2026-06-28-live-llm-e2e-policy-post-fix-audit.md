# Live LLM E2E Policy Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Git tree at start of this correction:
  `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Worktree status: dirty with prior audit/fix changes plus this e2e policy
  correction.

## Implemented Rule Correction

- `AGENTS.md` now states that e2e tests must be live LLM tests.
- E2E tests are forbidden from using scripted model clients, mocked provider
  responses, canned tool results, fake LLM outputs, or mock data standing in for
  a model decision.
- Deterministic scripted/model tests are explicitly limited to unit, adapter, or
  integration tiers.
- Live LLM e2e tests may remain `#[ignore]` by default when they require
  credentials, network access, or provider spend.

## Test Reclassification

- Removed the deterministic conformance e2e target:
  `crates/agent-os-conformance/tests/e2e_tests.rs`.
- Removed the deterministic e2e module directory:
  `crates/agent-os-conformance/tests/e2e/`.
- Moved the scripted goal-driven runtime coverage into the integration tier:
  `crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs`.
- Cleaned scripted/mock test names and audit-log labels so they no longer claim
  e2e semantics.

## Live LLM E2E Additions

- Added live OpenAI-compatible and Anthropic-compatible full tool-surface e2e
  tests:
  - `live_openai_compatible_llm_goal_driven_full_tool_surface_e2e`
  - `live_anthropic_compatible_llm_goal_driven_full_tool_surface_e2e`
- Added live OpenAI-compatible and Anthropic-compatible append-only-store
  rejection `agent_control` action e2e tests:
  - `live_openai_compatible_llm_goal_driven_agent_control_rejection_e2e`
  - `live_anthropic_compatible_llm_goal_driven_agent_control_rejection_e2e`
- The live success scenario expects a real provider to exercise all
  model-visible tool families and these `agent_control` actions:
  `start`, `status`, `output`, `set_hook`, `send`, `set_timeout`,
  `export_trace`, `resume`, `stop`, and `kill`.
- The live negative scenario expects real provider calls for append-only-store
  rejection actions: `delete_session` and `purge_state`.

## Current Live E2E Count

- Live LLM e2e tests are now 10 ignored tests, all under
  `crates/agent-os-thread/src/openai/tests/live.rs`.
- They remain ignored by default because they require live provider credentials
  and network access.

## Validation

- `cargo fmt`
- `cargo test -p agent-os-thread --lib --no-run`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

All non-live validation passed. Live ignored tests are compile-checked by the
Rust test build, but not executed without `LLM_API_KEY` and a reachable live
endpoint.

## Remaining Gaps

- Live LLM e2e behavior is inherently provider-dependent. The tests now encode
  the required live goals and assertions, but successful execution requires
  credentials and a sufficiently capable model.
- The repository still contains non-e2e unit, adapter, and integration tests
  that use mock/scripted clients. Those are intentionally retained outside the
  e2e tier.
