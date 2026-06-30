# Runtime Noncompletion Exit Code Pre-Fix Audit

Date: 2026-06-30

## Git Baseline

- HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`
- HEAD tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`
- Worktree status before this fix:

```text
 M Cargo.lock
 M crates/agent-os-cli/Cargo.toml
 M crates/agent-os-cli/src/args.rs
 M crates/agent-os-cli/src/chat.rs
 M crates/agent-os-cli/src/chat/tests.rs
 M crates/agent-os-cli/src/support.rs
 M crates/agent-os-conformance/tests/integration/artifact_conformance.rs
 M crates/agent-os-conformance/tests/integration/ecosystem_conformance.rs
 M crates/agent-os-conformance/tests/integration/lifecycle_conformance.rs
 M crates/agent-os-conformance/tests/integration/runtime_goal_driven_tools.rs
 M crates/agent-os-conformance/tests/integration/tool_broker.rs
 M crates/agent-os-kernel/src/profile_seed/tool_schemas.rs
 M crates/agent-os-kernel/src/profile_seed/tools/filesystem.rs
 M crates/agent-os-kernel/src/tools.rs
 M crates/agent-os-kernel/src/tools/driver/workspace.rs
 M crates/agent-os-thread/src/openai/messages.rs
 M crates/agent-os-thread/src/openai/prompt.rs
 M crates/agent-os-thread/src/openai/tests/unit.rs
 M crates/agent-os-thread/src/runtime.rs
 M crates/agent-os-thread/src/runtime/tests.rs
 M crates/agent-os-thread/src/runtime/tool_policy.rs
?? benchmarks/
?? docs/20-implementation/swe-bench-lite-private-benchmark.md
?? docs/audit/2026-06-29-recoverable-tool-failures-post-fix-audit.md
?? docs/audit/2026-06-29-recoverable-tool-failures-pre-fix-audit.md
?? docs/audit/2026-06-29-swe-bench-chat-blob-store-post-fix-audit.md
?? docs/audit/2026-06-29-swe-bench-chat-blob-store-pre-fix-audit.md
?? docs/audit/2026-06-30-chat-task-file-post-fix-audit.md
?? docs/audit/2026-06-30-chat-task-file-pre-fix-audit.md
?? docs/audit/2026-06-30-run-command-env-post-fix-audit.md
?? docs/audit/2026-06-30-run-command-env-pre-fix-audit.md
?? docs/audit/2026-06-30-runtime-context-projection-post-fix-audit.md
?? docs/audit/2026-06-30-runtime-context-projection-pre-fix-audit.md
?? docs/audit/2026-06-30-swebench-runner-env-isolation-post-fix-audit.md
?? docs/audit/2026-06-30-swebench-runner-env-isolation-pre-fix-audit.md
?? docs/audit/2026-06-30-swebench-task-prompt-post-fix-audit.md
?? docs/audit/2026-06-30-swebench-task-prompt-pre-fix-audit.md
?? logs/
```

The dirty files are pre-existing Agent-OS implementation, conformance, and
benchmark work. This audit will only modify the runtime noncompletion contract,
focused regression tests, and this audit pair.

## Audit Scope

Focused paths:

- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-conformance/tests/integration/ecosystem_conformance.rs`
- `crates/agent-os-cli/src/chat.rs`
- `crates/agent-os-cli/src/chat/tests.rs`

## Current-Contract Findings

1. Private SWE-bench Lite WSL2 records show several Agent-OS runs with
   `exit_code: 1` and empty final patches.
2. Representative logs show the first hard errors are runtime validation
   failures, not git diff collection failures:
   - `runtime received 2 consecutive model turns with no tool call or final submission`
   - `runtime reached max_steps without final submission`
3. `ThreadRuntime::run_to_completion_with_overrides` currently returns
   `AgentOsError::Validation` for both repeated no-action model turns and
   max-step exhaustion.
4. `agent-os chat --task-file` calls `session.process_task(...)?` in the
   initial-task path, so these runtime validation errors become process exit 1.
5. The interactive loop partially handles `max_steps`, but only after the
   initial task path. That leaves batch benchmark runs vulnerable to a normal
   noncompletion state being recorded as a process failure.

## Future-Roadmap Gaps

- Empty patches after a clean noncompletion report still represent agent/model
  performance gaps, prompt gaps, or task-environment gaps.
- This fix does not attempt to improve model convergence or patch quality for
  any SWE-bench task.

## Validation Already Run

- Inspected WSL2 benchmark records and logs from the local private run
  directory. Detailed local paths are intentionally kept in operator memory,
  not Git.
- Confirmed `django__django-14667` exits through the consecutive no-action
  validation path.
- Confirmed `django__django-16400`, `psf__requests-2148`,
  `pydata__xarray-4094`, `pylint-dev__pylint-6506`, and
  `sympy__sympy-12481` exit through the max-steps validation path.

## Intended Fix Scope

Apply a forward-only runtime contract:

- Provider/client, storage, permission, and unrecoverable kernel failures remain
  hard runtime errors.
- Repeated no-action model turns and step-limit exhaustion become normal
  noncompletion reports with `final_submitted: false`.
- The kernel marks the thread and task `Blocked` with a clear reason when the
  runtime stops for these noncompletion reasons.
- Batch `agent-os chat --task-file` exits normally after writing its report and
  optional bundle, allowing benchmark runners to distinguish "attempt completed
  without final patch" from "process crashed".
- Focused tests are added before production code changes.
