# Runtime Context Projection Pre-Fix Audit

## Baseline

- Timestamp: 2026-06-30T01:55:03.8998612+08:00
- Git HEAD: 97157d3bef1b927cb52a7a0d31af2a56bcf059e9
- Git tree: 8995e3d48c5d0e520a48e179325c0dd767349e43
- Worktree status: dirty; existing changes include Agent-OS CLI/runtime fixes, benchmark runner updates, benchmark artifacts, and audit documents.

## Audit Scope

This audit covers the Agent Thread Runtime projection and model-action loop behavior observed during the WSL2 SWE-bench Lite `django__django-14667` smoke run.

## Current-Contract Findings

1. The runtime projects all recent tool results, and also projects any older tool result with evidence.
2. Workspace tools attach evidence to large outputs such as full-file `read_file` results, so a large source file can remain in every later model turn.
3. In the clean `django__django-14667` smoke, the model read a large Django source file multiple times; later provider usage stayed around 13k input tokens per turn while the model repeatedly emitted long output text.
4. Several turns consumed thousands of output tokens but produced no parseable action, recorded as `(no action from model)`, wasting the finite step budget.
5. The task made no workspace edits before the smoke was stopped, so the current runtime behavior is not competitive enough for a SWE-bench/OpenCode comparison.

## Future-Roadmap Gaps

- Richer file-reading tools such as line-ranged reads may be useful, but this audit focuses on the runtime's current projection and loop contract rather than adding a broad new workspace tool family.

## Validation Already Run

- Windows and WSL runner tests pass after the benchmark environment isolation fix.
- `django__django-14667` clean smoke avoided the previous stale-state lease conflict but still entered a long no-edit/no-action loop.
- The invalid smoke was stopped and the temporary provider credential file was
  removed.

## Intended Fix Scope

- Add focused runtime tests for bounded projection of older evidence-bearing tool outputs.
- Add focused runtime tests for no-action model responses consuming a turn with a concise corrective tool-style result instead of silently allowing long unproductive loops.
- Keep the public tool names and core kernel authority model unchanged.
- Keep changes forward-only: no compatibility flags, fallback modes, or legacy projection path.
