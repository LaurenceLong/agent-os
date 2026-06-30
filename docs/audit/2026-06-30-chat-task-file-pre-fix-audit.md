# Chat Task File Pre-Fix Audit

Date: 2026-06-30

## Git Baseline

- HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`
- HEAD tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`

## Worktree Context

The tree is already dirty from the SWE-bench private benchmark setup, Linux
runner work, CLI blob-store fix, and recoverable tool-failure fix. This audit
only covers the additional `agent-os chat --task-file` CLI contract.

## Audit Scope

Focused files:

- `crates/agent-os-cli/src/args.rs`
- `crates/agent-os-cli/src/chat.rs`
- `benchmarks/swe-bench-lite/private20_runner.py`
- `benchmarks/swe-bench-lite/tests/test_private20_runner.py`

## Current-Contract Findings

1. `agent-os chat` accepts initial task text through `--task`, `-t`, or a
   positional task.
2. The SWE-bench private runner currently passes the whole task prompt as the
   `--task` argument. That exposes long problem statements in process command
   lines and stores the full prompt inside runner command records.
3. Large SWE-bench prompts should be file artifacts, not process arguments.
   The file path is enough for auditability because the prompt file is already
   written under the run artifact directory.
4. This is a forward-only CLI ergonomics and safety contract. No legacy
   compatibility path is required beyond preserving the existing `--task`
   behavior.

## Future-Roadmap Gaps

- Other CLI entrypoints such as `run` and `code` may eventually benefit from
  file-backed task input too, but this benchmark blocker is specific to live
  `chat`.
- The runner still needs full Agent-OS and OpenCode 20-task execution plus
  official SWE-bench scoring before behavioral optimization can be targeted.

## Validation Already Run

- `python benchmarks/swe-bench-lite/tests/test_private20_runner.py`
- `wsl.exe -d Ubuntu-22.04 -- bash -lc 'cd /mnt/d/work/ai_agents/coding-agent/agent-os; python3 benchmarks/swe-bench-lite/tests/test_private20_runner.py'`
- Agent-OS Linux Django smoke through the new runner produced a correct patch.
- Official SWE-bench harness scored that runner-produced patch as resolved
  `1/1`.

## Intended Fix Scope

- Add `--task-file` to `ChatOptions`.
- Reject simultaneous `--task` and `--task-file` for a single initial chat
  task.
- Read the task file before starting the runtime loop.
- Update `agent-os help` JSON.
- Update the private runner to pass `--task-file` instead of embedding the
  prompt text in the process command.
- Add focused unit tests for parser behavior and runner command construction.
