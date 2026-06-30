# run_command Environment Pre-Fix Audit

## Baseline

- Timestamp: 2026-06-30T02:00:01.2563577+08:00
- Git HEAD: 97157d3bef1b927cb52a7a0d31af2a56bcf059e9
- Git tree: 8995e3d48c5d0e520a48e179325c0dd767349e43
- Worktree status: dirty; this session already contains benchmark runner, task-file, recoverable tool failure, and runtime projection changes.

## Audit Scope

This audit covers the model-visible `run_command` tool input contract and shell driver behavior for per-command environment variables.

## Current-Contract Findings

1. `run_command` accepts only `program`, `args`, and injected `cwd`.
2. SWE-bench repositories often require per-command environment variables for focused test commands, for example `PYTHONPATH=.` when invoking Django test scripts.
3. Without a structured `env` field, the model must discover host-specific wrappers such as `env PYTHONPATH=. python ...`, which is less reliable and less portable than the Agent-OS tool schema owning the contract directly.

## Future-Roadmap Gaps

- Full process sandboxing and allowlisted environment variable policy can be refined later. The immediate need is deterministic per-command environment injection through the existing kernel-owned tool driver.

## Validation Already Run

- Runtime projection tests pass after adding no-action feedback and compact older evidence projection.
- Benchmark runner tests pass on Windows and WSL after environment isolation and stale state cleanup fixes.

## Intended Fix Scope

- Add an optional `env` object to the `run_command` model schema and kernel descriptor.
- Validate that env keys and values are strings.
- Apply env variables to the spawned process without invoking a shell.
- Update prompt wording so models know to use `env` for per-command variables.
- Add focused tests for the driver and model-visible schema.
