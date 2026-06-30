# Recoverable Tool Failures Post-Fix Audit

Date: 2026-06-29

## Git Baseline

- HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`
- HEAD tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`

## Implemented Fixes

1. Changed kernel tool invocation handling so input schema failures, driver
   failures, and output schema failures are recorded as failed terminal tool
   invocations with structured model-visible output.
2. Kept the kernel as the source of truth for tool lifecycle events by emitting
   `ToolCallFailed` and storing the failed `ToolExecutionRecord` instead of
   letting recoverable model-originated tool errors abort the runtime.
3. Updated Agent Thread hydration so failed, denied, cancelled, and timed-out
   terminal tool results are replayed to the model on resume, not only completed
   tool calls.
4. Narrowed the `run_command` nonzero-exit policy to completed process records
   so failed schema or driver records can be returned to the model for recovery.
5. Added runtime regression coverage for failed edit recovery, failed
   `run_command` recovery, and resume hydration of persisted failed tool
   results.
6. Updated conformance expectations for the new forward-only contract: current
   model-visible tool failures are failed invocations, not hard runtime errors.

## Changed Files

- `crates/agent-os-kernel/src/tools.rs`
- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-thread/src/runtime/tool_policy.rs`
- `crates/agent-os-conformance/tests/integration/artifact_conformance.rs`
- `crates/agent-os-conformance/tests/integration/ecosystem_conformance.rs`
- `crates/agent-os-conformance/tests/integration/lifecycle_conformance.rs`
- `crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs`
- `docs/audit/2026-06-29-recoverable-tool-failures-pre-fix-audit.md`
- `docs/audit/2026-06-29-recoverable-tool-failures-post-fix-audit.md`

## Validation Results

```text
cargo fmt --all --check
result: passed

cargo test -p agent-os-thread
result: 35 passed; 10 ignored

cargo test -p agent-os-kernel
result: passed

cargo test -p agent-os-conformance
result: 78 passed

cargo test --workspace
result: passed

cargo clippy --workspace --all-targets -- -D warnings
result: passed
```

## Benchmark Rerun Evidence

Representative reruns were executed with:

```text
model: tongyi/qwen3.6-plus
api style: anthropic-compatible
base URL: redacted private endpoint
```

The provider API key was written only to a temporary provider config during
execution and was deleted after the reruns. Detailed benchmark result rows and
local artifact paths are intentionally kept out of Git. They belong in local
operator memory and ignored run directories.

## Forward-Only Notes

The new contract treats recoverable model-originated tool failures as canonical
tool results. This is not a compatibility shim for old behavior; it is the
current runtime contract required by the normal Agent Thread loop and prompt.

Provider/client failures, corrupted storage, and unrecoverable kernel failures
remain hard errors. A failed tool invocation is only model-visible when the
kernel can record a terminal invocation and preserve replay determinism.

## Remaining Gaps

1. The full private 20-task run has not been rerun after this fix; only three
   representative tasks were rerun to validate the failure-recovery contract.
2. Official SWE-bench scoring remains unavailable on this Windows host because
   Docker and the SWE-bench harness are not installed.
3. Some tasks still reach `max_steps` without final submission. That points to
   model convergence, prompt, or task-environment gaps beyond this runtime
   recovery fix.
4. CLI progress logging around native runtime errors is still sparse when the
   process exits through a top-level validation error.
