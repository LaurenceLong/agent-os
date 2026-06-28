# Agent-OS Fix Audit

Date: 2026-06-28

## Code Baseline And Fix Hashes

- Base Git HEAD: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Base Git tree: `a6f51e395b6aa9d0445c4e952aecd7fd8b408d7f`
- Tracked patch-id before this post-fix document: `d2c6b2c5cc10f2f27512b73cfc44e5356e76f0a4`
- Source aggregate SHA-256 before this post-fix document:
  `34b00777dbdc042ea449c50d107a629b7d27b3ec5c864421edcb7e2527b727e1`

The aggregate hash was computed over tracked files plus untracked, non-ignored
files after the code fixes and before writing this post-fix audit document.

## Implemented Fixes

1. Added `submit_final` to the kernel tool descriptor registry.
   The lifecycle tool is now discoverable in the kernel tool surface and has a
   Tool Broker driver that records a `FinalSubmitted` event when invoked with an
   evidence map. The existing runtime fast path for `ModelAction::Final` remains
   intact for backward compatibility.

2. Added a session lifecycle tool driver.
   `submit_final` now validates structured input through the normal descriptor
   schema path, builds a `FinalSubmission`, applies the existing final evidence
   gate, and returns a structured tool result.

3. Made `agent_control` lifecycle actions stateful where safe.
   `resume` transitions the target to `Ready`; `stop` and `kill` terminate the
   target through validated thread transitions; `set_timeout` updates the target
   ATCB wall-time budget through a durable `ThreadConfigured` event; `output`
   returns provider stream events for the target; `export_trace` returns
   replayable events related to the target.

4. Stopped pretending destructive state deletion is implemented.
   `delete_session` and `purge_state` now record a rejected control command and
   return `Unsupported` because the v0.1 store is append-only and has no safe
   deletion/purge semantics.

5. Added kernel-side risk enforcement for privileged `agent_control` actions.
   The driver now rejects privileged or higher-impact actions when a caller
   understates risk level, even if a hand-written tool invocation bypasses the
   model adapter's risk mapping.

6. Hid privileged `agent_control` actions from normal WorkerAgent tool views.
   OpenAI-compatible and Anthropic-compatible tool definitions are now generated
   from the current thread role. Worker and Reviewer views omit `kill`,
   `delete_session`, and `purge_state`; Supervisor views retain them.

7. Documented the required audit workflow in `AGENTS.md`.
   Future audit-driven fixes now have an explicit `audit -> document -> fix ->
   document` process and must write audit records under `docs/audit/`.

## Changed Files

- `AGENTS.md`
- `crates/agent-os-conformance/tests/artifact_conformance.rs`
- `crates/agent-os-conformance/tests/lifecycle_conformance.rs`
- `crates/agent-os-kernel/src/profile_seed/tools.rs`
- `crates/agent-os-kernel/src/tools/driver.rs`
- `crates/agent-os-kernel/src/tools/driver/agent_control.rs`
- `crates/agent-os-kernel/src/tools/driver/session.rs`
- `crates/agent-os-thread/src/openai/client.rs`
- `crates/agent-os-thread/src/openai/tests/unit.rs`
- `crates/agent-os-thread/src/openai/tools.rs`
- `docs/audit/2026-06-28-pre-fix-design-audit.md`
- `docs/audit/2026-06-28-post-fix-audit.md`

## Validation

- `cargo fmt`: passed
- `cargo test --workspace`: passed
  - 79 non-ignored tests passed.
  - 6 live LLM tests remained ignored because they require real provider
    credentials.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed

## Compatibility Notes

- No SQLite schema migration was added.
- No persisted event shape was removed or changed.
- `submit_final` tool registration is additive.
- Runtime final submission behavior remains compatible with existing model
  adapters.
- Worker model tool views are stricter for privileged `agent_control` actions,
  matching the documented contract.
- `delete_session` and `purge_state` now fail closed instead of returning a
  placeholder success. This is an intentional safety correction for append-only
  state.

## Remaining Gaps

The following gaps from the pre-fix audit remain open:

- Scheduler admission is still not fully connected to budgets, provider slots,
  human attention, and resource pressure.
- Runtime checkpointing still does not occur at every documented yield boundary.
- Recovery still does not reconcile orphan running tools or workspace diffs.
- Final verification still does not parse all high-impact final claims or
  enforce stale-evidence and verification-state coverage comprehensively.
- Context projection, memory write policy, and compaction provenance remain
  skeletal.
- Store traits are still narrower than the full design's store-family list.
- Provider credential resolution, quota policy, retry policy, transforms, and
  provider-slot admission remain incomplete.
- The official software-engineering distribution package boundary is still
  represented by helper code rather than a full distro manifest and policy pack.

## Follow-Up Risk

`agent_control.rs` is now 565 lines after adding stateful lifecycle behavior. It
is below the repository's 600-line split-before-substantial-growth threshold,
but future Agent Control expansion should extract action-specific helpers before
adding more responsibilities.
