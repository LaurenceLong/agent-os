# Helper Function Policy Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-helper-function-policy-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Updated `AGENTS.md` so helper functions are not an accepted implementation
  pattern.
- Replaced helper-module and helper-fixture wording with fixture/module wording
  where the policy is about test organization.
- Removed the prior AGENTS forward-only audit wording that allowed helper
  functions when they reduced complexity.
- Inlined the external model binary compilation fixture steps in
  `agent-os-thread` and `agent-os-cli` tests, removing `compile_helper`,
  `rustc_path`, and `helper` local variable naming.
- Removed the `agent_control` "helper action" error string wording.
- Reworded older audit entries that used "helper" for private functions or
  hard-coded pipelines.

## Changed Files

- `AGENTS.md`
- `crates/agent-os-cli/src/run/tests.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`
- `crates/agent-os-thread/src/external.rs`
- `docs/audit/2026-06-28-agent-control-split-post-fix-audit.md`
- `docs/audit/2026-06-28-agents-forward-only-rules-pre-fix-audit.md`
- `docs/audit/2026-06-28-agents-forward-only-rules-post-fix-audit.md`
- `docs/audit/2026-06-28-helper-function-policy-pre-fix-audit.md`
- `docs/audit/2026-06-28-helper-function-policy-post-fix-audit.md`
- `docs/audit/2026-06-28-live-e2e-rejection-naming-pre-fix-audit.md`
- `docs/audit/2026-06-28-post-fix-audit.md`
- `docs/audit/2026-06-28-pre-fix-design-audit.md`
- `docs/audit/2026-06-28-roadmap-gaps-pre-fix-audit.md`
- `docs/audit/2026-06-28-roadmap-gaps-post-fix-audit.md`

## Forward-Only Notes

The current implementation style is direct by default. Extracted functions
should be named domain operations, ownership boundaries, or independently
testable contracts rather than generic helpers.

## Validation Results

- `rg -n "compile_helper|\\bhelper\\b|helper function|helper action|helper modules|helpers are allowed|Introduce helper|Keep helpers" crates AGENTS.md --glob '!target/**'`:
  only the negative `AGENTS.md` rule remains.
- `cargo test -p agent-os-thread external_process_client --lib`: passed.
- `cargo test -p agent-os-cli cli_run_can_use_external_model_process`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.

## Remaining Gaps

No current-contract helper-function policy gaps remain in this audit scope.
