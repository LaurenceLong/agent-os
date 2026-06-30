# SWE-bench Runner Environment Isolation Post-Fix Audit

Date: 2026-06-30

## Implemented Fixes

1. Scrubbed parent virtualenv state from benchmark child processes by removing
   `VIRTUAL_ENV`, `PYTHONPATH`, `PYTHONHOME`, and virtualenv `bin`/`Scripts`
   path entries.
2. Injected each task workspace as `PYTHONPATH` for both Agent-OS and OpenCode
   benchmark commands so local project imports resolve to the checked-out task
   repository.
3. Isolated Agent-OS provider config per task with a temporary
   `XDG_CONFIG_HOME`.
4. Removed stale Agent-OS SQLite sidecars (`.sqlite`, `.sqlite-wal`,
   `.sqlite-shm`) before each task run.
5. Added per-task timeout handling that records exit code `124` rather than
   leaving the runner blocked on a long model session.
6. Added `--resume-existing` support so interrupted benchmark runs can continue
   from already recorded task JSON files.
7. Fixed the OpenCode invocation order so the task message is passed directly to
   `opencode run` and the prompt file remains an attached file.

## Changed Files

- `benchmarks/swe-bench-lite/private20_runner.py`
- `benchmarks/swe-bench-lite/tests/test_private20_runner.py`

## Validation Results

```text
python benchmarks\swe-bench-lite\tests\test_private20_runner.py
result: 13 passed

wsl.exe -d Ubuntu-22.04 --exec bash -lc "cd /mnt/d/work/ai_agents/coding-agent/agent-os && python3 benchmarks/swe-bench-lite/tests/test_private20_runner.py"
result: 13 passed
```

## Benchmark Evidence

The WSL2/Docker SWE-bench harness was used for local validation after the runner
environment isolation fix. Detailed benchmark result rows, generated patches,
official harness reports, and local run paths are intentionally kept out of Git
and stored as local operator evidence.

## Forward-Only Notes

Benchmark child process isolation is a current-contract requirement. The runner
must evaluate the checked-out task repository, not a globally editable package
or the runner's own virtualenv.

## Remaining Gaps

- The official SWE-bench evaluator can still stall on individual instances; keep
  scoring resumable and instance-scoped instead of relying only on a monolithic
  all-predictions evaluation.
