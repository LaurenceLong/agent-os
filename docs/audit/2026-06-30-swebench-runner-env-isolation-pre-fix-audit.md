# SWE-bench Runner Environment Isolation Pre-Fix Audit

## Baseline

- Timestamp: 2026-06-30T01:39:47.2037533+08:00
- Git HEAD: 97157d3bef1b927cb52a7a0d31af2a56bcf059e9
- Git tree: 8995e3d48c5d0e520a48e179325c0dd767349e43
- Worktree status: dirty; existing changes include Agent-OS CLI/runtime fixes, benchmark artifacts, audit documents, and generated SWE-bench reports.

## Audit Scope

This audit covers the private SWE-bench Lite runner environment contract for live Agent-OS and OpenCode task runs under WSL2.

## Current-Contract Findings

1. The runner invokes Agent-OS and OpenCode task processes with a direct copy of the runner process environment.
2. The official SWE-bench helper process runs inside `/root/agent-os-swebench-venv`, so task tools inherit `VIRTUAL_ENV`, venv `PATH`, and any Python site-package state from previous smoke runs.
3. In the aborted Agent-OS 20-task run, `django__django-14667` repeatedly ran Python/Django inspection commands and eventually hit `max_steps` without final submission.
4. The state event log showed `python tests/runtests.py ...` importing Django from a previous smoke workspace path instead of the active task workspace, proving cross-task Python environment contamination.
5. The aborted run is not a valid benchmark result and should not be compared with OpenCode or official SWE-bench scoring.
6. Reusing a task output root after an aborted run can also reuse the task state sqlite file, leaving stale active provider leases that make the next run fail with `ResourceConflict("resource lease conflict resolved as denial")`.

## Future-Roadmap Gaps

- The runner does not yet create per-task language dependency environments. That is useful later, but not required for a clean model/agent comparison because official scoring runs in SWE-bench harness containers.

## Validation Already Run

- WSL2 Agent-OS binary build and task-file smoke passed earlier in this session.
- The local smoke and aborted run records are operator evidence and are kept out
  of Git.
- The full Agent-OS 20-task run was stopped after detecting environment contamination.
- The temporary provider credential file was removed after stopping the invalid run.

## Intended Fix Scope

- Add focused runner tests proving task subprocesses receive a cleaned benchmark environment.
- Remove Python virtualenv markers and venv path entries from Agent-OS/OpenCode task subprocess environments.
- Preserve explicit runner-owned environment additions such as Agent-OS `XDG_CONFIG_HOME`.
- Remove stale Agent-OS state sqlite sidecar files before starting a task run in an existing output root.
- Keep the shared prompt, task manifest, and official scoring contract unchanged.
