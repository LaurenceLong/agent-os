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

- Unit tests cover local parsing, schema mapping, permission boundaries, and
  failure modes.
- Integration/conformance tests cover crate boundaries and durable contract
  behavior.
- Live LLM tests must use real provider responses and keep mock tests clearly
  labeled as adapter/unit tests.
- Goal-driven live tests should use the normal system prompt and normal runtime
  loop. They may construct a workspace scenario and task goal, but must not add
  hidden per-tool instructions that force a specific call sequence.
- Any change to prompts, tool schemas, parser behavior, runtime loop, or provider
  adapters must include focused tests and should emit inspectable audit logs when
  the behavior is hard to see otherwise.
- Run the relevant test suite and `cargo clippy --workspace --all-targets -- -D
  warnings` before handing off when feasible.

## Working With Dirty Trees

- Assume unrelated local modifications are user work. Do not revert them.
- Keep edits scoped to the requested task.
- If a file is already dirty, understand the existing changes before editing and
  preserve unrelated work.
- Do not use destructive git commands unless the user explicitly asks for them.

