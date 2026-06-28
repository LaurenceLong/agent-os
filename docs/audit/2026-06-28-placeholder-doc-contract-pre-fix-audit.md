# Placeholder Doc Contract Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the in-progress forward-only
  roadmap implementation, conformance layout migration, provider-doc sync,
  helper-policy correction, and audit records.

## Scope

This audit covers non-audit design documentation that still uses placeholder
language for current-contract Provider System and Agent Thread tool execution
behavior.

## Current-Contract Gaps

- `docs/10-kernel-design/provider-system.md` says the model names in the
  provider configuration are placeholders, even though the current kernel seed
  data already defines active model aliases and provider routing validates them.
- `docs/10-kernel-design/agent-thread-core-module.md` lists "sandbox selection
  placeholder" as an Agent Thread tool-loop deliverable, even though workspace
  tool execution now depends on active environment leases and sandbox profiles.

These statements make current-contract documentation read as a draft rather
than as the forward-only architecture now implemented in the tree.

## Future-Roadmap Gap

No placeholder documentation item is intentionally deferred in this scope.

## Intended Fix Scope

- Rewrite the Provider System configuration example to use current seed IDs and
  active alias names.
- Replace placeholder wording with current model-catalog/alias enforcement
  language.
- Replace Agent Thread sandbox placeholder wording with explicit sandbox profile
  resolution and enforcement language.

## Validation Planned

- `rg -n "placeholder|placeholders" docs crates AGENTS.md distros --glob '!docs/audit/**' --glob '!target/**'`
- `cargo fmt --check`
- `git diff --check`
