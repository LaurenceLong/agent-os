# Recoverable Tool Failures Pre-Fix Audit

Date: 2026-06-29

## Git Baseline

- HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`
- HEAD tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`
- Worktree status before this fix:

```text
 M Cargo.lock
 M crates/agent-os-cli/Cargo.toml
 M crates/agent-os-cli/src/support.rs
?? benchmarks/
?? docs/20-implementation/swe-bench-lite-private-benchmark.md
?? docs/audit/2026-06-29-swe-bench-chat-blob-store-post-fix-audit.md
?? docs/audit/2026-06-29-swe-bench-chat-blob-store-pre-fix-audit.md
```

The dirty files above are from the SWE-bench private benchmark setup, report,
and the CLI blob-store fix required before the benchmark could run.

## Audit Scope

This audit covers Agent Thread runtime handling of model-visible tool failures
observed while running the private SWE-bench Lite 20-task benchmark with
`tongyi/qwen3.6-plus`.

Focused source paths:

- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-thread/src/runtime/tool_policy.rs`
- `crates/agent-os-kernel/src/tools.rs`
- `crates/agent-os-kernel/src/tools/driver/workspace.rs`
- `crates/agent-os-thread/src/openai/messages.rs`
- `crates/agent-os-thread/src/openai/prompt.rs`

## Current-Contract Findings

1. The system prompt explicitly tells the model to continue from tool failure
   output: "if a tool fails, use the failure output to choose the next smallest
   corrective step."
2. Runtime design docs require the tool loop to record tool results and
   evidence, and the runtime transition contract says
   `WaitingTool -> Thinking` requires a tool result or failure event.
3. Kernel tool drivers already emit `ToolCallFailed` for driver and output
   schema failures after a `ToolCallStarted` event, but
   `Kernel::invoke_tool` returns the error to the runtime.
4. `ThreadRuntime::execute_tool_action` currently propagates that error with
   `?`, so a recoverable model action such as a bad `replace_text` match or
   missing process immediately aborts the Agent Thread instead of projecting a
   failed tool result into the next model request.
5. Tool input schema validation occurs before `ToolCallStarted`, so malformed
   model tool input such as `run_command.args` being an object instead of an
   array currently produces no replayable failed invocation for the model to
   inspect.
6. `hydrated_tool_results` only reloads completed tool invocations, so failed
   tool invocations are not projected on resume even when the kernel has
   recorded them.
7. `tool_policy::enforce` is still a thread-level abort path for
   `run_command` non-zero exit status. That policy may be useful for strict
   scripted tests, but it prevents SWE-bench style agents from iterating on
   failing test output when enabled by default.

## Benchmark Evidence

The first private SWE-bench Lite 20-task run recorded:

- 20 task records
- 0/20 final submissions accepted by Agent-OS
- 1/20 patches generated
- 19/20 no-patch runs

Representative runtime failures:

- `replace_text expected exactly one match, found 0`
- `replace_text expected exactly one match, found 2`
- `run process: program not found`
- `tool.input.args expected array`
- `runtime reached max_steps without final submission`

These failures are recoverable from the model's perspective when surfaced as
tool results. They should not be treated the same as provider stream failure,
kernel corruption, storage failure, or permission denial.

## Future-Roadmap Gaps

- Official SWE-bench grading still requires Docker or a dedicated SWE-bench
  harness environment; the current Windows host can only produce Agent-OS
  execution records and naive local test attempts.
- Longer-horizon benchmark quality still depends on better task-specific
  environment preparation for projects that require compiled extensions or old
  dependency stacks.
- Prompt tuning for patch discipline remains separate from the runtime failure
  contract fixed here.

## Validation Already Run

Before this audit, the CLI blob-store fix and benchmark setup had been verified
with:

- `cargo fmt --all --check`
- `cargo test -p agent-os-cli`
- `cargo clippy -p agent-os-cli --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

No runtime recovery fix has been applied yet in this audit.

## Intended Fix Scope

Apply a forward-only runtime contract:

- Model-originated tool invocation failures are canonical tool results and are
  projected back to the model.
- Kernel remains the source of truth for persisted tool invocation lifecycle
  events.
- Driver failures that already produce `ToolCallFailed` are converted into a
  `ToolExecutionRecord` instead of aborting the runtime.
- Input validation failures produce a failed tool invocation with structured
  error output before returning to the model.
- Resumed runtime context includes failed, denied, cancelled, and timed-out tool
  results as replayable context, not only completed tool calls.
- Provider/client failures, unrecoverable kernel/storage failures, and
  authorization denials remain hard runtime errors unless the kernel records a
  model-visible denial result.

Focused tests will be added before production code changes.
