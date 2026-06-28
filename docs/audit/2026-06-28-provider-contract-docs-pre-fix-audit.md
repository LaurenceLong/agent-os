# Provider Contract Docs Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress roadmap-gap fix
  set, conformance-layout migration, agent-control split changes, and audit
  records.

## Scope

This continuation aligns provider contract documentation with the current
forward-only provider implementation.

The maintainer directs this build to use one canonical forward design with no
fallbacks, compatibility aliases, legacy adapters, or migration shims. Current
code already removed provider fallback-chain types and events, but several
normative docs still require fallback behavior.

## Current-Contract Gaps

- `docs/10-kernel-design/provider-system.md` still states that the Provider
  System owns fallback policy, includes `fallback_chain` in the profile example,
  includes `ProviderFallback` in the normalized event families, and requires
  durable fallback events.
- `docs/10-kernel-design/kernel-data-model.md` still lists
  `fallback_chain: string[]` on `ProviderProfile`, while
  `crates/agent-os-sys/src/provider.rs` now has `allowed_model_aliases`,
  `credential_ref`, `retry_policy`, and `transform_policy` instead.
- `docs/10-kernel-design/system-architecture.md` still requires fallback
  events.
- `docs/20-implementation/conformance-and-quality.md` still lists provider
  fallback without durable event as a gate.
- `docs/30-decisions/ADR-0007-provider-system-is-global-control-plane.md`
  still presents fallback policy and fallback event emission as required
  provider milestones.
- Follow-up scan also found provider fallback wording in
  `docs/10-kernel-design/agent-thread-core-module.md` and
  `docs/10-kernel-design/state-storage-and-replay.md`.

## Intended Fix Scope

- Rewrite provider docs to use retry policy and fail-closed stream failure as
  the canonical behavior.
- Remove `fallback_chain`, `ProviderFallback`, and compatibility alias language
  from normative provider docs.
- Update provider data-model docs to match current Rust provider structs.
- Update adjacent roadmap and storage/replay docs that enumerate provider
  policy or provider events.
- Keep provider adapter-style names such as OpenAI-compatible and
  Anthropic-compatible where they describe wire protocols rather than backward
  compatibility shims.

## Validation Planned

- `rg` checks for removed fallback contract terms in provider-related docs.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
