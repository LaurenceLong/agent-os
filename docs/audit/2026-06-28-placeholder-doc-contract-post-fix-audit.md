# Placeholder Doc Contract Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-placeholder-doc-contract-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Rewrote the Provider System configuration example from draft placeholder
  values to current seed IDs:
  `prov_default`, `route_default`, `mock-provider`, `coding-primary`,
  `review-primary`, `mock-model`, and `text-only`.
- Replaced placeholder model-name wording with the current Model Catalog
  contract: provider routing resolves through active provider profile, routing
  policy, allowed alias list, and active alias record before opening a stream
  session.
- Replaced the Agent Thread "sandbox selection placeholder" deliverable with
  explicit active execution environment lease resolution and sandbox profile
  enforcement before workspace or process driver execution.

## Changed Files

- `docs/10-kernel-design/provider-system.md`
- `docs/10-kernel-design/agent-thread-core-module.md`
- `docs/audit/2026-06-28-placeholder-doc-contract-pre-fix-audit.md`
- `docs/audit/2026-06-28-placeholder-doc-contract-post-fix-audit.md`

## Forward-Only Notes

The design docs now describe the current canonical provider and sandbox
contracts instead of leaving placeholder language for future compatibility or
fallback interpretation.

## Validation Results

- `rg -n "placeholder|placeholders" docs crates AGENTS.md distros --glob '!docs/audit/**' --glob '!target/**'`:
  no matches.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract placeholder documentation gaps remain for this audit scope.
