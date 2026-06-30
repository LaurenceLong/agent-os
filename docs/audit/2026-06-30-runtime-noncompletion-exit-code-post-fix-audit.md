# Runtime Noncompletion Exit Code Post-Fix Audit

Date: 2026-06-30

## Git Baseline

- HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`
- HEAD tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`

## Implemented Fixes

1. Changed `ThreadRuntime` so repeated no-action model turns and max-step
   exhaustion return a normal `RuntimeRunReport` with `final_submitted: false`
   instead of bubbling `AgentOsError::Validation` to the CLI process.
2. Added a single runtime noncompletion path that marks the task and thread
   `Blocked` with an explicit reason and preserves provider sessions, tool
   results, artifacts, checkpoints, and event counts for audit and resume.
3. Updated kernel thread transitions so `ThreadStatus::Blocked` closes the
   active turn as `TurnStatus::Blocked` instead of leaving it `InProgress`.
4. Updated conformance expectations that intentionally stop after one runtime
   step to assert the new blocked-report contract.
5. Added runtime regression coverage for both observed benchmark failure modes:
   consecutive text-only/no-action model responses and max-step exhaustion
   after a tool action without final submission.

## Changed Files

- `crates/agent-os-thread/src/runtime.rs`
- `crates/agent-os-thread/src/runtime/tests.rs`
- `crates/agent-os-kernel/src/threads.rs`
- `crates/agent-os-conformance/tests/integration/ecosystem_conformance.rs`
- `docs/audit/2026-06-30-runtime-noncompletion-exit-code-pre-fix-audit.md`
- `docs/audit/2026-06-30-runtime-noncompletion-exit-code-post-fix-audit.md`

## Validation Results

```text
cargo test -p agent-os-thread runtime_returns_blocked_report -- --nocapture
result: passed; 2 passed

cargo test -p agent-os-thread runtime_ -- --nocapture
result: passed; 11 passed

cargo test -p agent-os-kernel
result: passed; 8 passed

python benchmarks\swe-bench-lite\tests\test_private20_runner.py
result: passed; 13 passed

cargo test -p agent-os-conformance
result: passed; 78 passed

cargo test -p agent-os-cli
result: passed; 22 passed

cargo fmt --all --check
result: passed

cargo test --workspace
result: passed; 154 passed, 10 ignored

cargo clippy --workspace --all-targets -- -D warnings
result: passed

wsl.exe -d Ubuntu-22.04 --exec bash -lc "cd /mnt/d/work/ai_agents/coding-agent/agent-os && CARGO_TARGET_DIR=target/wsl2-linux cargo test -p agent-os-thread runtime_returns_blocked_report -- --nocapture && CARGO_TARGET_DIR=target/wsl2-linux cargo build -p agent-os-cli --bin agent-os"
result: passed; WSL2 focused test 2 passed and Linux debug binary built

wsl.exe -d Ubuntu-22.04 --exec bash -lc "cd /mnt/d/work/ai_agents/coding-agent/agent-os && ./target/wsl2-linux/debug/agent-os --help >/tmp/agent-os-wsl-help.json && sed -n '1,20p' /tmp/agent-os-wsl-help.json"
result: passed; Linux binary emitted help JSON
```

## Benchmark Regression Evidence

The original Agent-OS private-20 records contained 10 instances with
`exit_code: 1` and empty patches. After this fix, those original failing
instances were rerun one by one against the current Linux `agent-os` binary.
Detailed benchmark result rows and local artifact paths are intentionally kept
out of Git. They belong in local operator memory and ignored run directories.

## Forward-Only Notes

This is the current runtime contract, not a compatibility shim. Agent-OS now
distinguishes system/runtime failures from normal agent noncompletion:

- Provider/client errors, storage errors, permission failures, and unrecoverable
  kernel errors remain hard errors.
- Model non-action loops and step-budget exhaustion are auditable blocked
  runtime reports that allow batch runners to record a completed attempt without
  misclassifying it as a process crash.

## Remaining Gaps

1. The old SWE-bench Lite private-20 report still contains historical
   `exit_code: 1` records. The detailed regression run records remain local;
   the published comparison report can be regenerated separately when a full
   benchmark refresh is desired.
2. Empty patches after a blocked or completed runtime report remain model convergence,
   prompt, or task-environment failures; this fix only repairs the process
   contract.
3. Live LLM e2e tests remain ignored by default unless live provider
   credentials are supplied.
