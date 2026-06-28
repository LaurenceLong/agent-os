# Compatibility Layer Docs Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-compatibility-layer-docs-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Rewrote ADR-0003 so external agent frameworks can run only as distribution
  packages or external tools that use Agent-OS kernel contracts directly.
- Rewrote the Agent Thread Runtime purpose text to remove compatibility-layer
  hosting language.
- Replaced the foundation "Compatibility Principle" with an "Ecosystem Boundary
  Principle".
- Replaced compatibility distributions, gates, labels, and reports with
  conformance-oriented package boundary language.
- Preserved the rule that external packages cannot define Agent Thread
  lifecycle, syscall semantics, evidence semantics, permission semantics, kernel
  state layout, or scheduling contracts.

## Changed Files

- `docs/30-decisions/ADR-0003-agent-thread-runtime-is-proprietary-core.md`
- `docs/10-kernel-design/agent-thread-runtime.md`
- `docs/00-foundation/architecture-principles.md`
- `docs/20-implementation/production-roadmap.md`
- `docs/audit/2026-06-28-compatibility-layer-docs-pre-fix-audit.md`
- `docs/audit/2026-06-28-compatibility-layer-docs-post-fix-audit.md`

## Forward-Only Notes

This is an intentional documentation contract change. The project direction is
external integration through current kernel contracts and conformance gates, not
through alternate execution or compatibility layers.

## Validation Results

- `rg -n "compatibility layer|compatibility layers|compatibility distribution|compatibility distributions|compatibility gate|compatibility gates|compatibility label|compatibility labels|compatibility report|backward compatibility|fallback|legacy|shim|deprecated|temporary workaround|feature flag" docs crates AGENTS.md distros --glob '!docs/audit/**'`: only matched the `AGENTS.md` prohibition rule.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract compatibility-layer documentation gaps remain for this audit
scope.
