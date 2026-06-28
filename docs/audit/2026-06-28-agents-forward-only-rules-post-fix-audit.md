# AGENTS Forward-Only Rules Post-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at pre-fix audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Pre-fix audit:
  `docs/audit/2026-06-28-agents-forward-only-rules-pre-fix-audit.md`
- Worktree status after the fix: uncommitted modifications and new files remain.

## Implemented Fix

- Reframed `AGENTS.md` from an already-released project posture to a
  greenfield, forward-only project posture.
- Replaced the old compatibility rule section with non-negotiable forward-only
  rules.
- Removed instructions to preserve fallbacks, compatibility aliases, legacy code
  paths, old public API shapes, and older persisted-state compatibility.
- Added explicit rules against fallbacks, compatibility layers, legacy adapters,
  deprecated paths, migration shims, feature flags, and temporary workarounds.
- Added the maintainer's direct-code guidance: extracted functions should
  represent domain operations or meaningful boundaries, not generic helper
  functions.
- Updated storage rules so current schemas and current event streams remain
  deterministic while older persisted-state compatibility is not required.
- Updated the audit workflow wording from compatibility notes to forward-only
  notes.

## Changed Files

- `AGENTS.md`
- `docs/audit/2026-06-28-agents-forward-only-rules-pre-fix-audit.md`
- `docs/audit/2026-06-28-agents-forward-only-rules-post-fix-audit.md`

## Forward-Only Notes

This is an intentional project-rule contract change. Future changes should use
the current architecture and current schemas as the source of truth instead of
preserving old behavior.

## Validation Results

- `rg -n "already released|not as a greenfield|Backward compatibility|backward compatibility|Compatibility Rules|compatibility aliases|Compatibility shims|Keep fallbacks|legacy code paths|tolerate older records|compatibility notes|migration plan|currently unsupported" AGENTS.md`: no matches.
- `cargo test --workspace`: passed. Non-ignored tests passed; 10 live LLM e2e
  tests remained ignored by default.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with only Git LF/CRLF warnings.

## Remaining Gaps

No current-contract `AGENTS.md` rule gaps remain for this audit scope.
