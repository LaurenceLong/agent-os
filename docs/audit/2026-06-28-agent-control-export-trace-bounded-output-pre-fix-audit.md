# Agent Control Export Trace Bounded Output Pre-Fix Audit

## Git State

- HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- HEAD tree: `c2c2cbcc1b84ea1eebc042a7c8af0a342615b295`
- Worktree status: dirty before this fix; the tree already contains the active roadmap/live-e2e changes plus unrelated user work. This audit scopes only the `agent_control export_trace` output contract and the tests/docs needed to validate it.

## Scope

- Audit the current model-visible output of `agent_control` `export_trace`.
- Preserve a clean forward-only contract: no compatibility aliases, no fallback full-event payload, and no legacy response shape.
- Keep the output useful for model planning and conformance assertions while preventing large event streams from flooding live LLM context.

## Current-Contract Gaps

1. `export_trace` returns the complete matching event stream as `output.events`.
2. Live OpenAI-compatible full tool-surface e2e reaches `max_steps` after `export_trace`; the returned trace payload is large enough to dominate subsequent model context and the model repeats `agent_control` actions instead of submitting a final answer.
3. The public conformance assertion currently depends on the full `events` array, so the test contract encourages the oversized model-visible response.

## Future-Roadmap Gaps

- Durable trace export handles, pagination, and separate artifact-backed downloads remain future work. This fix only defines the current model-visible contract.

## Validation Already Run

- `cargo test --workspace` passed before live LLM changes.
- `cargo test -p agent-os-thread live_openai_compatible -- --ignored --nocapture --test-threads=1` exposed the full-surface `max_steps` failure.
- Focused live retries showed `record_evidence` now succeeds and the remaining failure follows the large `export_trace` response.

## Intended Fix Scope

- Replace the full `events` array with bounded summary fields and a small event preview.
- Update conformance assertions to use the bounded current-contract fields.
- Re-run focused conformance, live OpenAI-compatible e2e, workspace tests, clippy, formatting, and diff checks.
