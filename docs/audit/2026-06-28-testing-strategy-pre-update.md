# Testing Strategy Pre-Update Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Git tree: `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Worktree status: dirty from the prior design-audit fix set and existing
  `docs/audit/` records.
- Test marker count before this update: 110 `#[test]` or `#[ignore]` markers.

## Scope

This audit covers the test strategy rules and the immediate test coverage gaps
called out by the maintainer:

- Keep a test mix near 60% unit, 25% integration, and 15% e2e.
- Organize test files by tier and scenario.
- Add integration tests, because the current tree has conformance tests but no
  explicit integration-test tier.
- Add goal-driven e2e tests that exercise every model-visible tool and every
  `agent_control` action/subcommand.

## Current-Contract Gaps

- `AGENTS.md` does not define the target unit/integration/e2e ratio or how to
  judge deviations.
- Test organization does not expose an explicit `integration/` or `e2e/`
  directory structure. Existing conformance tests are useful but mostly flat.
- Runtime-level scripted tests cover a normal goal loop, but not the full
  model-visible tool surface.
- Adapter-level mock tests exercise most core tools, but they are parser/client
  tests rather than e2e runtime contract tests.
- `agent_control` actions are not all covered by goal-driven runtime tests:
  `status`, `output`, `set_hook`, `send`, `resume`, `stop`, `set_timeout`,
  `export_trace`, `kill`, `delete_session`, and `purge_state` need explicit
  coverage in addition to `start`.
- `submit_final` needs coverage through both paths: direct kernel tool broker
  integration and model-visible final submission through the runtime loop.

## Intended Fix Scope

- Expand `AGENTS.md` testing rules with the 60/25/15 target, tier definitions,
  file organization, and goal-driven e2e coverage requirements.
- Add explicit conformance integration-test entry points under an
  `integration/` module tree.
- Add explicit conformance e2e-test entry points under an `e2e/` module tree.
- Keep new tests deterministic and local. Live LLM tests remain ignored
  supplemental coverage, not the only e2e coverage.

## Validation Planned

- `cargo fmt`
- Focused conformance tests for the new integration and e2e targets
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings` when feasible
