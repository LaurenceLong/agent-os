# SWE-bench Chat Blob Store Pre-Fix Audit

Date: 2026-06-29

Git HEAD: `97157d3bef1b927cb52a7a0d31af2a56bcf059e9`

Git tree: `8995e3d48c5d0e520a48e179325c0dd767349e43`

Worktree status before fix:

```text
?? benchmarks/
?? docs/20-implementation/swe-bench-lite-private-benchmark.md
```

## Scope

Unblock running the private SWE-bench Lite benchmark through `agent-os-cli chat`
with a real Anthropic-compatible provider.

## Current-Contract Findings

1. `agent-os-cli chat` opens a kernel through `open_kernel()` but that helper
   returns a kernel without artifact or evidence blob stores.
2. Runtime tool calls attach inline evidence bytes. Without an evidence blob
   store, the first evidence-producing tool call fails with
   `evidence inline bytes require an evidence blob store`.
3. The failure occurs before benchmark task execution can start and is
   independent of the configured model endpoint.

## Future-Roadmap Gaps

1. The private SWE-bench runner still needs task checkout, dependency setup,
   result aggregation, and score reporting. This audit only covers the CLI
   kernel storage blocker encountered while starting the benchmark.

## Validation Already Run

```text
cargo run -p agent-os-cli --quiet -- chat ... --task "Create a file named smoke.txt ..."

Observed:
Error: Validation("evidence inline bytes require an evidence blob store")
```

## Intended Fix Scope

1. Add a regression test that proves `open_kernel()` returns a kernel that can
   attach inline artifact and evidence bytes.
2. Add local hash-addressed blob stores for CLI-opened kernels.
3. Keep provider credentials out of repository files and audit logs.
