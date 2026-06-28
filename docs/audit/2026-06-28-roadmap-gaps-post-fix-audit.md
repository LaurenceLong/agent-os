# Consolidated Roadmap Gaps Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-roadmap-gaps-pre-fix-audit.md`
- Worktree state after the fix: uncommitted modifications and new files remain.
  Existing agent-control split changes and its audit docs were already present
  in the local tree and are outside this roadmap-gap audit scope.

## Implemented Fixes

### Gap 1 - Scheduler Admission Wiring

- Made scheduler admission the canonical `turn.start` gate.
- Admission now evaluates goal, task, agent, provider-profile, and
  human-attention budget ledgers.
- Admission rejects unsatisfied task dependencies and provider-slot pressure
  before a turn starts.
- Ready transitions populate a scheduler queue ordered by task priority.
- Provider stream startup uses an exclusive provider-slot lease.

### Gap 2 - Runtime Checkpointing At Yield Boundaries

- Removed `RuntimeConfig.checkpoint_yield_boundaries`.
- Runtime checkpoints are now mandatory at yield boundaries.
- Side effects mark recovery state dirty, and committed checkpoints clear it.
- Final-session failure paths release provider slots by recording a failed
  stream session.
- The checkpoint implementation is direct at the yield sites; no checkpoint
  fallback path remains.

### Gap 3 - Recovery Reconciliation

- Added kernel reconciliation for orphan running tool calls.
- Added expired resource and environment lease reconciliation.
- Added workspace-diff reconciliation records and exposed their artifact refs
  through resume output.
- Resume now runs through the recovery reconciliation path before making the
  thread ready again.

### Gap 4 - Final Verification Depth

- Stale evidence is now a hard verification failure.
- High-impact final claims must be covered by active, non-stale evidence.
- `unverified_claims` no longer acts as a waiver for stale or uncovered
  high-impact claims.
- Final submission records a durable verification result before accepting the
  final answer.

### Gap 5 - Context Projection, Memory Policy, Compaction Provenance

- `ModelTurnRequest` now carries a bounded `ModelContextProjection` instead of
  full hydrated tool-result and artifact history.
- The projection includes recent context plus evidence-bearing tool results,
  artifacts, fresh contexts, active memory, and compaction records.
- `context.commit_summary`, `context.invalidate`, and `memory.*` are exposed
  through canonical syscall and permission policy surfaces.
- Memory provenance now validates active evidence ids.

### Gap 6 - Store-Family Trait Coverage

- `KernelStore` now includes the canonical projection and idempotency store
  families.
- Added projection store behavior for both in-memory and SQLite store drivers.
- Kept business logic against store traits rather than concrete database
  drivers.

### Gap 7 - Provider System

- `ProviderProfile` now carries an opaque `CredentialRef`, retry policy, and
  transform policy.
- Provider routing resolves credential references without exposing raw secrets
  to agent threads.
- Provider-profile quotas are enforced through provider-scoped ledgers.
- Provider slots are leased before stream startup and released on completion or
  failure.
- Runtime model calls retry according to provider-profile retry policy.
- Removed provider fallback-chain behavior and fallback route events.
- Removed OpenAI client-side ad hoc env probing and compatibility aliases; the
  CLI uses the canonical `LLM_*` configuration surface.

### Gap 8 - Software-Engineering Distribution

- Added `distros/software-engineering/` with manifest, role prompts, review
  policy, and final-answer policy.
- Added a distro loader in `agent-os-thread`.
- Refactored the software-engineering pipeline to require the distro package.
- The pipeline has no hard-coded distro fallback when the package is missing.

## Changed Files

Roadmap-gap implementation files:

- `crates/agent-os-cli/src/args.rs`
- `crates/agent-os-cli/src/chat.rs`
- `crates/agent-os-cli/src/chat/tests.rs`
- `crates/agent-os-cli/src/code.rs`
- `crates/agent-os-cli/src/resume.rs`
- `crates/agent-os-cli/src/run.rs`
- `crates/agent-os-conformance/tests/context_conformance.rs`
- `crates/agent-os-conformance/tests/resource_provider_storage_conformance.rs`
- `crates/agent-os-conformance/tests/runtime_resume_conformance.rs`
- `crates/agent-os-conformance/tests/software_distribution_conformance.rs`
- `crates/agent-os-kernel/src/artifacts.rs`
- `crates/agent-os-kernel/src/context.rs`
- `crates/agent-os-kernel/src/events.rs`
- `crates/agent-os-kernel/src/inputs.rs`
- `crates/agent-os-kernel/src/lib.rs`
- `crates/agent-os-kernel/src/profile_seed/permissions.rs`
- `crates/agent-os-kernel/src/profile_seed/provider.rs`
- `crates/agent-os-kernel/src/provider.rs`
- `crates/agent-os-kernel/src/recovery.rs`
- `crates/agent-os-kernel/src/scheduler.rs`
- `crates/agent-os-kernel/src/state.rs`
- `crates/agent-os-kernel/src/syscall.rs`
- `crates/agent-os-kernel/src/threads.rs`
- `crates/agent-os-kernel/src/verification.rs`
- `crates/agent-os-store-sqlite/src/lib.rs`
- `crates/agent-os-store-sqlite/src/projection.rs`
- `crates/agent-os-store/src/memory.rs`
- `crates/agent-os-store/src/traits.rs`
- `crates/agent-os-sys/src/context.rs`
- `crates/agent-os-sys/src/provider.rs`
- `crates/agent-os-thread/src/external.rs`
- `crates/agent-os-thread/src/lib.rs`
- `crates/agent-os-thread/src/model.rs`
- `crates/agent-os-thread/src/openai/client.rs`
- `crates/agent-os-thread/src/openai/messages.rs`
- `crates/agent-os-thread/src/openai/parser.rs`
- `crates/agent-os-thread/src/openai/tests.rs`
- `crates/agent-os-thread/src/openai/tests/live.rs`
- `crates/agent-os-thread/src/openai/tests/mock_adapter.rs`
- `crates/agent-os-thread/src/openai/tests/support.rs`
- `crates/agent-os-thread/src/openai/tests/unit.rs`
- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-thread/src/scripted.rs`
- `crates/agent-os-thread/src/software/distro.rs`
- `crates/agent-os-thread/src/software/mod.rs`
- `crates/agent-os-thread/src/software/pipeline.rs`
- `crates/agent-os-thread/src/software/roles.rs`
- `crates/agent-os-thread/src/software/tests.rs`
- `crates/agent-os-thread/src/software/types.rs`
- `distros/software-engineering/manifest.json`
- `distros/software-engineering/policy/final-answer.json`
- `distros/software-engineering/policy/review.json`
- `distros/software-engineering/prompts/reviewer.md`
- `distros/software-engineering/prompts/supervisor.md`
- `distros/software-engineering/prompts/worker.md`
- `docs/audit/2026-06-28-roadmap-gaps-pre-fix-audit.md`
- `docs/audit/2026-06-28-roadmap-gaps-post-fix-audit.md`

Pre-existing local changes observed but not attributed to this audit:

- `crates/agent-os-kernel/src/tools/driver/agent_control.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/action.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/command.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/hooks.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/lifecycle.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/mod.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control/target.rs`
- `docs/audit/2026-06-28-agent-control-split-pre-fix-audit.md`
- `docs/audit/2026-06-28-agent-control-split-post-fix-audit.md`

## Forward-Only Notes

The maintainer explicitly directed this build to use a greenfield,
forward-only contract. The following old paths were intentionally removed:

- Removed the runtime checkpoint switch.
- Removed provider route-switching API and projection behavior.
- Removed OpenAI ad hoc env probing in favor of canonical `LLM_*` config.
- Changed `ModelTurnRequest` to use `context: ModelContextProjection`.
- Promoted `context.commit_summary` as the canonical compaction syscall with no
  alternate syscall name.
- Required the software-engineering distro package instead of retaining a
  hard-coded pipeline path.

No database schema migration was required by this change set. Store trait
surfaces changed, and SQLite projection behavior was added behind the current
driver boundary.

## Validation Results

- `cargo fmt`: passed.
- `cargo fmt --check`: passed after final line-ending cleanup.
- `git diff --check`: passed after final line-ending cleanup.
- `cargo check --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.

Focused suites also passed during the fix:

- `cargo test -p agent-os-conformance --test resource_provider_storage_conformance`
- `cargo test -p agent-os-conformance --test context_conformance`
- `cargo test -p agent-os-conformance --test runtime_resume_conformance`
- `cargo test -p agent-os-conformance --test software_distribution_conformance`
- `cargo test -p agent-os-conformance --test integration_tests`
- `cargo test -p agent-os-thread runtime --lib`
- `cargo test -p agent-os-kernel verification --lib`

## Remaining Gaps

No current-contract gaps remain for the eight audited roadmap items.

The live LLM e2e tests are still ignored by default because they require live
provider credentials, network access, and possible provider spend. They should
be run separately before a live-provider release gate.
