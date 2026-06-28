# Compatibility Layer Docs Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress roadmap-gap fix
  set, conformance-layout migration, provider-doc sync, AGENTS rule sync,
  agent-control split changes, and audit records.

## Scope

This audit covers non-audit architecture and roadmap docs that still present
compatibility layers, compatibility distributions, or compatibility reporting as
an intended Agent-OS direction.

The maintainer directs this build to use one greenfield, forward-only design and
not to add compatibility layers.

## Current-Contract Gaps

- `docs/30-decisions/ADR-0003-agent-thread-runtime-is-proprietary-core.md`
  still says open-source agents may be supported as guest runtimes or
  compatibility layers.
- `docs/10-kernel-design/agent-thread-runtime.md` still says external agent
  systems can be hosted behind compatibility layers.
- `docs/00-foundation/architecture-principles.md` still has a "Compatibility
  Principle" and lists compatibility distributions and compatibility gates.
- `docs/20-implementation/production-roadmap.md` still lists compatibility
  labels and a compatibility report as ecosystem deliverables.

These are documentation-level conflicts with the current no-compatibility-layer
objective.

## Intended Fix Scope

- Rewrite compatibility-layer wording to conformance and package-boundary
  wording.
- Keep the architecture rule that external packages cannot define kernel
  lifecycle, syscall, evidence, permission, state, or scheduling contracts.
- Rename compatibility labels/reports/gates to conformance labels/reports/gates.
- Do not introduce any new adapter or wrapper concept.

## Validation Planned

- `rg` checks for compatibility-layer terminology outside audit records.
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
