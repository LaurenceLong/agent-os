# Live LLM E2E Policy Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Git tree: `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Worktree status: dirty with the prior design-audit fix set, testing-rule
  updates, deterministic runtime tests currently under an `e2e` path, and audit
  records.

## Scope

This audit corrects the e2e test definition. The maintainer clarified that e2e
tests must be live LLM tests only. Deterministic, scripted, mock, or canned model
data is forbidden in the e2e tier.

## Findings

- `AGENTS.md` currently allows deterministic scripted or mock model clients for
  local e2e coverage. This conflicts with the clarified rule.
- `crates/agent-os-conformance/tests/e2e/goal_driven_tools.rs` uses
  `ScriptedModelClient`, so it is not a valid e2e test under the clarified rule.
- Existing live LLM tests are all under `crates/agent-os-thread/src/openai/tests/live.rs`
  and are marked `#[ignore]` because they require provider credentials and a live
  endpoint.
- Existing live goal-driven coverage does not yet include a single full
  tool/action surface scenario.

## Intended Fix Scope

- Update `AGENTS.md` so e2e means live LLM only.
- Move deterministic runtime coverage out of the e2e test target and into the
  integration tier.
- Add live LLM goal-driven e2e tests for the full model-visible tool/action
  surface.
- Keep live tests ignored by default because they require external credentials,
  while making them the only tests allowed to use e2e naming.

## Validation Planned

- `cargo fmt`
- `cargo test --workspace` for non-live tests
- `cargo test -p agent-os-thread --lib -- --ignored` is not expected to run in
  this environment without `LLM_API_KEY`; compile coverage comes from
  `cargo test --workspace` and clippy.
- `cargo clippy --workspace --all-targets -- -D warnings`
