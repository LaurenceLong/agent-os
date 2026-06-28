# Testing Strategy Post-Fix Audit

Date: 2026-06-28

Correction note: the e2e classification in this document was superseded by
`2026-06-28-live-llm-e2e-policy-post-fix-audit.md`. Deterministic scripted
runtime coverage is now integration coverage, and e2e is live LLM only.

## Code Identity

- Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Git tree at start of this testing update:
  `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Worktree status: dirty with the prior design-audit implementation, this
  testing update, and audit records.
- Test marker count after this update: 113 `#[test]` or `#[ignore]` markers.

## Implemented Rule Updates

- Expanded `AGENTS.md` testing rules with the target mix of roughly 60% unit,
  25% integration, and 15% e2e coverage by meaningful scenarios/assertions.
- Defined tier ownership and layout:
  - unit tests close to crate code,
  - integration tests under `crates/agent-os-conformance/tests/integration/`,
  - e2e tests under `crates/agent-os-conformance/tests/e2e/`.
- Required goal-driven e2e coverage for every model-visible tool and every
  `agent_control` action, including privileged success, denial, and currently
  unsupported append-only-store paths.
- Required `submit_final` coverage through both kernel tool-broker integration
  and runtime final-submission paths.

## Implemented Tests

- Added `crates/agent-os-conformance/tests/integration_tests.rs` as the
  integration test target.
- Added `crates/agent-os-conformance/tests/integration/tool_broker.rs`.
  This exercises the kernel tool broker across all model-visible tool families:
  workspace tools, process execution, work-state tools, evidence recording,
  communication tools, `agent_control start`, and direct `submit_final`.
- Added `crates/agent-os-conformance/tests/e2e_tests.rs` as the e2e test target.
- Added `crates/agent-os-conformance/tests/e2e/goal_driven_tools.rs`.
  This uses the normal runtime loop with a deterministic scripted model client
  to exercise:
  - `read_file`, `write_file`, `replace_text`, `delete_file`, `run_command`,
  - `set_objective`, `update_checklist`, `record_evidence`,
  - `report_supervisor`, `post_blackboard`, `ask_human`,
  - `agent_control` actions `start`, `status`, `output`, `set_hook`, `send`,
    `set_timeout`, `export_trace`, `resume`, `stop`, and `kill`,
  - runtime final submission, covering model-visible `submit_final` semantics.
- Added a separate goal-driven e2e rejection test for `agent_control kill`
  without sufficient action risk and for `delete_session` / `purge_state`, which
  are intentionally rejected by the append-only v0.1 store.

## Supporting Runtime Change

- Added `RuntimeRunOverrides` and
  `ThreadRuntime::run_to_completion_with_overrides`.
- The default `RuntimeConfig` shape and `run_to_completion` behavior remain
  unchanged for compatibility.
- Overrides allow tests or callers to provide a bounded approval id and sandbox
  profile override when deliberately exercising high-risk runtime paths.

## Compatibility Notes

- No existing public `RuntimeConfig` fields were removed or changed.
- The final implementation avoids adding fields to `RuntimeConfig`, because
  that would break downstream struct-literal construction.
- Existing role defaults remain intact. Supervisor remains read-only by default;
  the e2e test explicitly overrides the sandbox profile for its bounded
  high-coverage scenario.
- High-risk `agent_control` e2e coverage uses real kernel approvals instead of
  bypassing the permission model.

## Validation

- `cargo fmt`
- `cargo test -p agent-os-conformance --test integration_tests --test e2e_tests`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

All validation commands passed.

## Remaining Gaps

- Existing flat conformance files remain in place for compatibility and should
  be migrated gradually only when touching their scenarios.
- Live LLM e2e tests remain ignored supplemental coverage; deterministic local
  e2e now covers the full tool/action surface for CI.
- The 60/25/15 split is now documented as a target, but it is intentionally not
  enforced by a brittle raw-count gate.
