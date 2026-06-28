# Consolidated Roadmap Gaps Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at original audit start: clean.
- Current scope amendment: the maintainer now directs this build to treat the
  repository as a greenfield, unreleased system.
- Test baseline at original audit start: 107 non-ignored tests passed, 10 live
  LLM e2e tests ignored, 0 failed.

## Audit Scope

This iteration advances the eight roadmap-level gaps that prior audits
(`2026-06-28-post-fix-audit.md`, `2026-06-28-agent-control-split-post-fix-audit.md`)
carried forward as remaining work.

Scope amendment after maintainer directive: this repository is treated as a
greenfield, unreleased system for this build. The fix scope must choose the
cleanest forward-only design and must not preserve backward compatibility,
legacy adapters, fallbacks, migration shims, feature flags, compatibility
aliases, or temporary workarounds. Existing code that conflicts with the
intended architecture should be refactored or removed instead of adapted
around. Deterministic replay still matters for the current design, but there
is no requirement to keep old event shapes, old public APIs, or previous
runtime behavior.

## Current-Contract Gaps

### Gap 1 - Scheduler Admission Wiring

- Contract: `docs/10-kernel-design/scheduler-and-resource-arbitration.md`
  defines budget admission, resource pressure, provider slots, ready queues,
  and first-production scheduler behavior.
- Current: `start_turn` rejects busy statuses, and in-progress work has added
  partial budget and ready-queue checks. Provider-slot pressure,
  human-attention pressure, and queue priority remain incomplete.
- Intended fix scope: make scheduler admission the canonical `turn.start`
  path. It must consult goal/task/agent/provider/human-attention budget
  ledgers, task DAG readiness, and provider-slot pressure; populate and order
  the ready queue on `Ready` transitions; and return machine-readable
  rejection reasons. There is no legacy path that starts a turn without
  scheduler admission.

### Gap 2 - Runtime Checkpointing At Yield Boundaries

- Contract: `docs/10-kernel-design/agent-thread-runtime.md` requires
  checkpointing at documented yield boundaries.
- Current: in-progress work added checkpoint calls but retained a
  `checkpoint_yield_boundaries` compatibility switch and still writes an
  extra legacy final checkpoint.
- Intended fix scope: make checkpointing mandatory at documented yield
  boundaries. Remove compatibility switches and single-checkpoint behavior;
  mark `ThreadRecovery.dirty = true` after side effects and `false` only when
  a checkpoint is committed.

### Gap 3 - Recovery Reconciliation

- Contract: `docs/10-kernel-design/agent-thread-core-module.md` requires
  restart reconciliation of orphan running tools, workspace diffs, and expired
  leases.
- Current: in-progress work adds orphan-tool and expired-lease reconciliation,
  but workspace-diff reconciliation is not represented and the resume path is
  only partly wired.
- Intended fix scope: add a kernel recovery pass that marks orphan running
  tools interrupted/cancelled, reclaims expired resource and environment
  leases, records workspace-diff reconciliation, and runs as the only resume
  path before a thread is made ready again.

### Gap 4 - Final Verification Depth

- Contract: `docs/10-kernel-design/kernel-data-model.md` requires every
  high-impact final claim to be checked and stale evidence to fail
  verification.
- Current: in-progress work adds keyword checks and stale metadata checks, but
  the high-impact waiver semantics are confused and tests currently cover a
  workaround rather than the stated contract.
- Intended fix scope: make final verification a single authoritative gate:
  parse high-impact claims from the final submission, require each to have
  active non-stale evidence, treat `unverified_claims` as explicit non-final
  disclosures rather than pass-through waivers, and record a durable
  `Verification` before `FinalSubmitted`.

### Gap 5 - Context Projection, Memory Policy, Compaction Provenance

- Contract: `docs/10-kernel-design/agent-thread-core-module.md` and
  `docs/10-kernel-design/kernel-data-model.md` require scoped projections,
  stale marking, provenance, and non-authoritative proposed memory.
- Current: in-progress work adds memory and compaction methods, but runtime
  model requests still pass full hydrated tool/artifact history as the
  effective context.
- Intended fix scope: add the missing syscalls and make runtime model requests
  consume a bounded context projection as the canonical path. Full-history
  request hydration is not retained as a fallback.

### Gap 6 - Store-Family Trait Coverage

- Contract: `docs/10-kernel-design/state-storage-and-replay.md` lists the
  durable store families.
- Current: in-progress work adds projection-family traits but leaves
  `KernelStore` as a narrow subset.
- Intended fix scope: make the store traits represent the canonical store
  surface, with in-memory and SQLite implementations for projection queries.

### Gap 7 - Provider System

- Contract: `docs/10-kernel-design/provider-system.md` requires credential
  resolution, quota, retry, transforms, provider slots, and secret isolation.
- Current: credentials are read ad hoc by the OpenAI client; provider slots
  are not leased before streams; provider-scoped quotas and retry policy are
  not enforced.
- Intended fix scope: add a single canonical provider path:
  `ProviderProfile` carries an opaque `CredentialRef`, routing resolves
  credentials without exposing raw secrets to Agent Threads, provider-scoped
  ledgers enforce quota, provider slots are leased before a stream opens and
  released on completion, retry policy is explicit in provider/profile config,
  and provider transforms are applied by adapter style. Remove ad hoc
  client-side env probing and compatibility aliases.

### Gap 8 - Software-Engineering Distribution

- Contract: `docs/10-kernel-design/system-architecture.md`,
  `docs/10-kernel-design/kernel-abi-and-syscalls.md`,
  `docs/20-implementation/production-roadmap.md`, and
  `docs/30-decisions/ADR-0009-v0-1-core-surface-convergence.md` require the
  official software-engineering workflow to live behind a distro package
  boundary, not as kernel-hard-coded workflow.
- Current: `crates/agent-os-thread/src/software/` is a hard-coded workflow
  pipeline driven by `ScriptedModelClient`; no distro manifest or policy pack
  exists.
- Intended fix scope: add a `distros/software-engineering/` package with a
  manifest, workflow prompts, review policy pack, and final-answer policy
  pack; add a `DistroLoader` in `agent-os-thread`; and refactor the software
  pipeline to require that distro package. There is no hard-coded workflow
  fallback when the distro is absent.

## Verification At Audit Start

- `cargo check --workspace`: passed during this continuation after observing
  the in-progress roadmap-gap changes.
- Earlier baseline before this continuation recorded:
  - `cargo build --workspace`: passed.
  - `cargo test --workspace`: passed (107 passed, 10 ignored, 0 failed).
  - `cargo clippy --workspace --all-targets -- -D warnings`: passed.

## Forward-Only Strategy

Every change in this iteration must satisfy:

1. There is one canonical implementation path for each contract. Remove
   compatibility switches, legacy aliases, deprecated branches, fallback
   loaders, and temporary workarounds when they obscure the intended
   architecture.
2. Public APIs, serialized shapes, and store traits may change when that makes
   the greenfield design clearer. Tests must be updated to the new contract.
3. Replay determinism is preserved for the current event model. Historical
   compatibility with old event streams is not a requirement in this build.
4. SQLite migrations remain deterministic and versioned only when physical
   schema changes are introduced.
5. Each gap ships with focused tests at the right tier (unit for pure logic,
   integration for cross-crate contracts, no mock data under e2e).
