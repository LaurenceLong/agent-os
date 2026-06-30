# Runtime Context Projection Post-Fix Audit

Date: 2026-06-30

## Implemented Fixes

1. Added compact projection for older evidence-bearing tool results so large
   command or file outputs do not remain fully expanded in every later provider
   request.
2. Added `runtime_feedback` as an internal projected feedback item for model
   turns that return text without a tool call or final submission.
3. Rendered runtime feedback as user text in OpenAI-compatible message
   projection, avoiding fake tool-call history.
4. Added a current-contract cap of two consecutive no-action model turns; the
   runtime now exits with a validation error instead of burning the remaining
   step budget.

## Changed Files

- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-thread/src/openai/messages.rs`
- `crates/agent-os-thread/src/openai/tests/unit.rs`

## Validation Results

```text
cargo test -p agent-os-thread runtime_ -- --nocapture
result: 10 passed

cargo fmt --all --check
result: passed

cargo clippy --workspace --all-targets -- -D warnings
result: passed
```

## Forward-Only Notes

The runtime now has one canonical behavior for no-action model responses:
surface concise feedback once, then fail after repeated no-action turns. This is
not a provider-specific fallback and does not add a compatibility mode.

## Remaining Gaps

- More precise file-windowing tools could further improve SWE-bench efficiency,
  but that is a separate tool-surface design rather than part of this runtime
  projection fix.
