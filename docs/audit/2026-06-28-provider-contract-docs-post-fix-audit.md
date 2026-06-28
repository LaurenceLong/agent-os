# Provider Contract Docs Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-provider-contract-docs-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Rewrote provider normative docs from fallback policy to retry policy and
  fail-closed stream failure.
- Removed `fallback_chain` from the documented `ProviderProfile` shape.
- Documented the current provider profile fields:
  `allowed_model_aliases`, `credential_ref`, `retry_policy`, and
  `transform_policy`.
- Removed `ProviderFallback` from the normalized stream event list.
- Updated provider conformance expectations to require durable retry/failure
  recording and provider-slot lease release.
- Updated adjacent Agent Thread and storage/replay docs so provider policy and
  event examples no longer name the removed route-switching path.
- Updated ADR-0007 so required provider milestones match the current
  forward-only implementation.
- Updated architecture and quality-gate docs to refer to retry and stream
  failure events rather than provider fallback events.

## Changed Files

- `docs/10-kernel-design/provider-system.md`
- `docs/10-kernel-design/kernel-data-model.md`
- `docs/10-kernel-design/agent-thread-core-module.md`
- `docs/10-kernel-design/state-storage-and-replay.md`
- `docs/10-kernel-design/system-architecture.md`
- `docs/20-implementation/conformance-and-quality.md`
- `docs/30-decisions/ADR-0007-provider-system-is-global-control-plane.md`
- `docs/audit/2026-06-28-provider-contract-docs-pre-fix-audit.md`
- `docs/audit/2026-06-28-provider-contract-docs-post-fix-audit.md`

## Forward-Only Notes

This is an intentional forward-only contract update. The provider system now
documents one route per stream session, explicit retry policy, fail-closed
failure recording, and canonical `LLM_*` environment mapping.

## Validation Results

- `rg -n "fallback|fallback_chain|ProviderFallback|compatibility aliases|OpenAI-compatible variables" ...`: no matches in provider-related docs and provider implementation files.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract provider-doc gaps remain for this audit scope.
