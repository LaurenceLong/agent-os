# Agent-OS Project Rules

This repository is not released. Treat every change as part of a greenfield,
forward-only system design.

## Non-Negotiable Forward-Only Rules

- Choose the cleanest current contract over preserving old behavior.
- Do not add fallbacks, compatibility layers, legacy adapters, deprecated paths,
  migration shims, feature flags, or temporary workarounds.
- When existing code conflicts with the intended architecture, refactor or
  remove it instead of adapting around it.
- Prefer one canonical implementation path over multiple supported paths.
- Public APIs, CLI flags, environment variables, serialized JSON shapes, tool
  names, provider adapter styles, and profile identifiers may change when the
  forward design is clearer.
- Database and event-store changes must preserve deterministic behavior for the
  current schema and current event model. Historical compatibility with older
  persisted state is not required for this build.

## Architecture Boundaries

- `agent-os-sys` owns shared ABI/data types. Keep it dependency-light and avoid
  runtime behavior there.
- `agent-os-kernel` owns authoritative state transitions, permissions,
  resources, communication, tools, evidence, artifacts, replay, and profile
  policy. Runtime crates may request kernel operations; they must not duplicate
  kernel authority.
- `agent-os-thread` owns model/client adapters and the Agent Thread runtime loop.
  It converts model actions into kernel calls, but the kernel remains the source
  of truth.
- `agent-os-store` and `agent-os-store-sqlite` own storage traits and drivers.
  Do not let business logic depend on a concrete database implementation.
- `agent-os-cli` owns user-facing command orchestration and formatting. Keep core
  behavior in library crates.
- Conformance tests describe the public contract. Update them when the contract
  changes.

## File And Module Size

- Keep production Rust modules focused and preferably under 400 lines.
- Files over 600 lines must be split before adding substantial new behavior.
- Files over 1000 lines are considered architectural debt. Do not add new
  responsibilities to them; extract cohesive modules first.
- Test files may be larger than production modules, but long tests must still be
  organized by scenario and fixture modules.
- Split by ownership, not by arbitrary line count. Good splits include:
  provider client, prompt/message construction, tool schema, parser, runtime
  loop, tool driver family, profile seed family, and test fixtures.
- Avoid dumping unrelated shared routines into `lib.rs`, `mod.rs`, or a broad
  `util.rs`. Facade modules should mostly declare modules and re-export stable
  types.

## Implementation Style

- Make small, coherent changes that match the existing crate boundary.
- Prefer typed structs/enums and serde-compatible schemas over ad hoc string
  parsing.
- Keep the implementation simple, direct, and explicit by default.
- Do not introduce helper functions as an implementation pattern. Keep logic in
  its owning function unless extraction creates a named domain operation,
  ownership boundary, or independently testable contract.
- Do not add wrappers, utility layers, adapters, or indirection merely to look
  more modular.
- Add comments only for non-obvious invariants, forward-only contract choices, or
  security-sensitive decisions.
- Do not introduce a new framework or runtime dependency for core Agent-OS
  behavior without an ADR.

## Storage And Migration Rules

- Schema changes must be versioned, deterministic, and tested when a physical
  schema migration exists.
- Persisted fields and event payloads should model the current contract directly.
- Deserializers should reject records that do not match the current contract.
- Replay must remain deterministic for current event streams.
- Before changing store schemas or event payloads, update conformance tests that
  exercise restart, replay, versioning, and idempotency.

## Testing Rules

- Maintain a practical test mix near 60% unit tests, 25% integration tests, and
  15% e2e tests by meaningful scenarios and assertions, not by brittle raw line
  counts. Document any material deviation when the risk profile calls for it.
- Unit tests cover local parsing, schema mapping, permission boundaries,
  deterministic reducers, and failure modes. Keep them close to the code under
  `src/**/tests.rs`, `src/**/tests/*.rs`, or tight `#[cfg(test)]` modules.
- Integration tests cover crate boundaries and durable contract behavior such as
  kernel plus store, kernel plus tool broker, runtime plus kernel, and CLI plus
  library orchestration. Organize new integration tests under
  `crates/agent-os-conformance/tests/integration/` with a small top-level test
  target that imports the directory modules.
- E2E tests must be live LLM tests. They cover normal user or agent goals
  through the regular runtime loop, normal system prompt, normal model/provider
  adapter, normal capability grants, and realistic persisted state.
- E2E tests must not use scripted model clients, mocked provider responses,
  canned tool results, fake LLM outputs, or mock data standing in for a model
  decision. Deterministic or mock model clients belong in unit, adapter, or
  integration tests only and must not live under `e2e` paths or use e2e names.
- E2E tests may prepare real local workspace fixtures and expected assertions,
  but every model action in the e2e path must come from a live provider
  response. Live LLM e2e tests may be `#[ignore]` by default when they require
  credentials, network access, or provider spend.
- Goal-driven e2e coverage must exercise every model-visible tool and every
  subcommand/action in that tool surface. For `agent_control`, cover `start`,
  `status`, `output`, `set_hook`, `send`, `resume`, `stop`, `set_timeout`,
  `export_trace`, `kill`, `delete_session`, and `purge_state`, including
  privileged success, denial, and append-only-store rejection paths.
- `submit_final` must be covered both as a kernel tool-broker integration path
  and as the model-visible final-submission path through the runtime loop.
- Goal-driven live tests should use the normal system prompt and normal runtime
  loop. They may construct a workspace scenario and task goal, but must not add
  hidden per-tool instructions that force a specific call sequence.
- Any change to prompts, tool schemas, parser behavior, runtime loop, provider
  adapters, storage schemas, migrations, or replay behavior must include focused
  tests at the right tier and should emit inspectable audit logs when the
  behavior is hard to see otherwise.
- Shared test fixtures live in `tests/common` or a narrow crate-local support
  module. Keep fixtures scenario-focused and avoid dumping unrelated utilities
  into broad fixture files.
- Run the relevant test suite and `cargo clippy --workspace --all-targets -- -D
  warnings` before handing off when feasible.

## Audit And Fix Workflow

- For design or implementation audits that lead to code changes, use the
  workflow `audit -> document -> fix -> document`.
- Store audit records under `docs/audit/`. Create the directory when it is
  missing.
- Before editing code, write a pre-fix audit document that records the current
  Git `HEAD`, Git tree hash, worktree status, audit scope, findings, validation
  already run, and the intended fix scope.
- Keep findings split between current-contract gaps and future-roadmap gaps.
  Do not treat documented out-of-scope roadmap items as immediate release
  blockers unless the task explicitly asks for them.
- After code changes, write a post-fix audit document that records the new
  validation results, changed files, implemented fixes, forward-only notes, and
  remaining gaps.
- When the audit changes public behavior, tool schemas, runtime behavior,
  storage behavior, or conformance expectations, update focused tests in the
  same change.

## Working With Dirty Trees

- Assume unrelated local modifications are user work. Do not revert them.
- Keep edits scoped to the requested task.
- If a file is already dirty, understand the existing changes before editing and
  preserve unrelated work.
- Do not use destructive git commands unless the user explicitly asks for them.

