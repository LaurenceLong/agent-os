# Agent-OS Design Audit Before Fixes

Date: 2026-06-28

Correction note: this pre-fix audit records the initial findings before the
forward-only roadmap work. Current closure status for findings 1, 4, 5, 7, 8,
9, and 10 is recorded in `2026-06-28-roadmap-gaps-post-fix-audit.md`. Current
project-rule posture is recorded in
`2026-06-28-agents-forward-only-rules-post-fix-audit.md`.

## Code Baseline

- Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Git tree: `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Worktree status at audit start: clean

## Audit Scope

Reviewed project documentation under `docs/`, the Rust workspace layout, kernel
state and syscall paths, tool broker drivers, Agent Thread runtime, OpenAI and
Anthropic-compatible model adapters, SQLite store, conformance tests, and CLI
surfaces.

The review compared implementation against the documented v0.1 baseline and the
larger Agent-OS design. Future roadmap items such as distributed scheduling,
remote workers, PostgreSQL production deployment, marketplace packages,
graphical consoles, and third-party certification are tracked as future design
gaps, not immediate compatibility defects.

## Verification At Audit Start

- `cargo test --workspace`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- Live LLM tests were not run; they are ignored by default and require real
  provider credentials.

## Findings

1. Scheduler admission is not complete.
   Budget ledgers and resource leases exist, but budget exhaustion, provider
   slot pressure, human attention, and resource availability are not consistently
   applied as turn/model/tool admission decisions.

2. `agent_control` is partially implemented.
   `start`, `status`, and `set_hook` have durable state behavior, but `output`,
   `send`, `resume`, `stop`, `set_timeout`, `export_trace`, `kill`,
   `delete_session`, and `purge_state` mostly record commands and return empty
   placeholder output.

3. Privileged `agent_control` actions are model-visible to normal workers.
   The model-facing tool schemas expose `kill`, `delete_session`, and
   `purge_state` in the normal OpenAI and Anthropic-compatible tool definitions.
   Documentation says those actions must be hidden from normal WorkerAgent tool
   views.

4. Runtime yield and recovery are incomplete.
   The runtime records checkpoints at final completion, but does not checkpoint
   at every documented yield boundary or reconcile orphan running tools and
   workspace state after restart.

5. Final verification remains shallow.
   `submit_final` enforces a non-empty active evidence map, but it does not yet
   parse high-impact claims, check stale evidence comprehensively, or require
   verification state for all final acceptance paths.

6. `submit_final` tool registration is inconsistent.
   `submit_final` is a model-visible lifecycle tool, but the kernel tool
   registry does not include a descriptor for it. Runtime final submission is
   handled directly rather than as a normal tool invocation, which should remain
   explicit and tested.

7. Context, memory, and compaction are skeletal.
   Context snapshots and memory ABI types exist, but typed context projection,
   memory proposal/activation policy, and compaction provenance are not yet
   complete.

8. Storage trait coverage is narrower than the full design.
   Current store traits cover events, idempotency, and blobs. The full design
   names additional store families for projections, locks, leases, profiles,
   scheduler state, messages, mementos, memory, provider state, and audit.

9. Provider System is an early control-plane skeleton.
   Provider profile resolution, aliasing, route events, and usage accounting
   exist, but credential resolution, quota policy, retry policy, transforms, and
   provider-slot admission are not complete.

10. The software-engineering distribution is still a hard-coded workflow
    pipeline.
    Existing code demonstrates the workflow, but the official distribution
    package, manifest, policy packs, prompt packs, and distro boundary are not
    complete.

## Fix Scope For This Turn

This turn will address findings that can be fixed safely without changing
persisted event shapes or introducing new storage migrations:

- Hide privileged `agent_control` actions from non-Supervisor model tool views.
- Implement durable, stateful behavior for common `agent_control` lifecycle
  actions where the existing ABI already supports it.
- Register `submit_final` in the kernel tool descriptor registry while keeping
  the existing lifecycle fast path during this initial fix.
- Add focused conformance/unit tests for the changed public contract.

The remaining findings were retained as design gaps at the time of this audit.
Their current status is superseded by later post-fix audit records.
