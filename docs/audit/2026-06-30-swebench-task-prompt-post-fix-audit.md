# SWE-bench Task Prompt Post-Fix Audit

Date: 2026-06-30

## Implemented Fixes

1. Tightened the benchmark task prompt to prohibit git history, previous
   commits, prior patches, and external sources for solving private tasks.
2. Added explicit hidden-test guidance: inspect local code and tests once, then
   solve from production behavior and the problem statement.
3. Kept the prompt benchmark-specific rather than changing the normal Agent-OS
   runtime prompt for all users.
4. Added runner tests asserting the hidden-test and source-control constraints
   remain present.

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

## Forward-Only Notes

The private benchmark prompt defines the current evaluation contract: solve from
the checked-out repository and problem statement, with no patch-history leakage.
It intentionally optimizes for fair private benchmark behavior rather than
preserving the earlier looser prompt.

## Remaining Gaps

- The prompt alone does not make weak model runs competitive; official scoring
  still shows failures from long investigations, no edits, and incomplete
  patches.
