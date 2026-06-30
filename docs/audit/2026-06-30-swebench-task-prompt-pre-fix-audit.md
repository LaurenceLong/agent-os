# SWE-bench Task Prompt Pre-Fix Audit

## Baseline

- Timestamp: 2026-06-30T03:13:32.3662027+08:00
- Git HEAD: 97157d3bef1b927cb52a7a0d31af2a56bcf059e9
- Git tree: 8995e3d48c5d0e520a48e179325c0dd767349e43
- Worktree status: dirty; this session already contains benchmark runner, CLI, runtime, and conformance changes.

## Audit Scope

This audit covers the shared private SWE-bench Lite task prompt generated for both Agent-OS and OpenCode benchmark runs.

## Current-Contract Findings

1. The prompt asks agents to inspect git diff before final submission but does not discourage git history exploration.
2. During the Agent-OS 20-task run, `django__django-16400` spent many turns exploring git history and repeating analysis without editing.
3. SWE-bench tasks should be solved from the current checked-out source, problem statement, and relevant tests; historical commits or prior patches are not part of the benchmark input contract.
4. The prompt also does not explicitly tell agents to submit final immediately after a focused fix, relevant validation, and diff inspection, which leaves room for repeated confirmation loops.

## Future-Roadmap Gaps

- Per-agent adaptive prompting may be useful later, but the current benchmark should keep one shared prompt for fair Agent-OS/OpenCode comparison.

## Validation Already Run

- Agent-OS run with the previous prompt solved the first four tasks and then stalled on `django__django-16400`.
- The stalled run was stopped and the temporary provider credential file was
  removed.

## Intended Fix Scope

- Update the shared prompt to ban git history/prior patch exploration unless explicitly requested by the task.
- Tell agents to keep investigation bounded once the relevant code path is found.
- Tell agents to submit final immediately after a scoped fix, relevant validation or captured blocker, and git diff inspection.
- Keep gold patches, hidden hints, and test patches excluded.
