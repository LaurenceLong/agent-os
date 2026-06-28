# Agent-OS Project Rules

This repository is already released. Treat every change as a maintained forward
change, not as a greenfield rewrite.

## Non-Negotiable Compatibility Rules

- Backward compatibility matters. Prefer the safest maintainable forward change
  that does not break existing users.
- Do not remove supported behavior, fallbacks, compatibility aliases, legacy
  code paths, or persisted-state handling unless the task explicitly requests it
  and includes a clear migration plan.
- Database and event-store changes require extra care. SQLite schemas,
  migrations, replay behavior, idempotency records, and persisted event payloads
  must remain readable by newer code unless an explicit migration plan says
  otherwise.
- Preserve existing public APIs, CLI flags, environment variables, serialized
  JSON shapes, tool names, provider adapter styles, and profile identifiers by
  default.
- Compatibility shims may be cleaned up only after the replacement path is
  documented, tested, and safe for existing state.

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
  organized by scenario and helper modules.
- Split by ownership, not by arbitrary line count. Good splits include:
  provider client, prompt/message construction, tool schema, parser, runtime
  loop, tool driver family, profile seed family, and test fixtures.
- Avoid dumping unrelated helpers into `lib.rs`, `mod.rs`, or a broad `util.rs`.
  Facade modules should mostly declare modules and re-export stable types.

## Implementation Style

- Make small, coherent changes that match the existing crate boundary.
- Prefer typed structs/enums and serde-compatible schemas over ad hoc string
  parsing.
- Keep compatibility aliases when accepting input from users, configs, or
  providers.
- Keep fallbacks that protect existing deployments unless a migration explicitly
  removes them.
- Add comments only for non-obvious invariants, compatibility behavior, or
  security-sensitive decisions.
- Do not introduce a new framework or runtime dependency for core Agent-OS
  behavior without an ADR.

## Storage And Migration Rules

- Schema migrations must be versioned, deterministic, and tested.
- New persisted fields should be additive where possible.
- Deserializers should tolerate older records when safe.
- Replay must remain deterministic for existing event streams.
- Before changing store schemas or event payloads, update conformance tests that
  exercise restart, replay, migration versioning, and idempotency.

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
  privileged success, denial, and currently unsupported append-only-store paths.
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
  module. Keep helpers scenario-focused and avoid dumping unrelated utilities
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
  validation results, changed files, implemented fixes, compatibility notes, and
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

