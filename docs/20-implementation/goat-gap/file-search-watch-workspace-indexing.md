# File Search, Watch, And Workspace Indexing Plan

Status: planning

Last updated: 2026-07-03

## Goal

Build a workspace discovery subsystem that supports fast file lookup, content
search, watch-driven invalidation, deterministic traversal, and cross-platform
path behavior without shelling out. The model-visible surface should stay small:
`glob_files` for paths and `grep_files` for content.

## Non-Goals

- Do not depend on host shell commands for core workspace discovery.
- Do not make a watcher bypass kernel resource policy.
- Do not introduce `find_files` as a third model-visible search tool.
- Do not preserve `workspace_discovery` as a standalone contract when
  `glob_files` and `grep_files` can own the behavior.

## Current Agent-OS State

The working tree currently has an in-progress split into:

- `glob_files`: path discovery by workspace-relative glob;
- `grep_files`: literal UTF-8 content search with optional include glob.

This is a good forward step away from shell search, but the gap remains:

- no watcher or invalidation stream;
- no runtime cancellation contract for active large traversals;
- no provider/runtime projection for changing workspace context.

Implemented behavior now includes deterministic bounded traversal, symlink
skipping, literal UTF-8 grep with binary/large-file skips, `.gitignore` and
`.ignore` rule loading, and default `target/` plus `node_modules/` directory
exclusion only when no `.gitignore` has been loaded for the current traversal
scope.

## Codex Reference

Codex uses file-search sessions with a walker thread and matcher thread,
gitignore-aware traversal, fuzzy ranking, query updates while walking,
cancellation checks, and result snapshots. Agent-OS does not need to copy the
fuzzy path tool surface. The useful Codex ideas are bounded traversal,
cancellation, ignore policy, and watch-driven invalidation. Codex also has a
multi-subscriber file watcher with recursive paths, RAII registrations,
debounced/throttled receivers, sorted coalesced event batches, and fsmonitor
safety logic.

## Target Agent-OS Contract

Agent-OS should make `glob_files` and `grep_files` the canonical search tools:

- workspace root identity and canonical path;
- ignore policy and traversal policy;
- bounded result windows and deterministic ordering;
- cancellation points for large traversals;
- watch subscriptions with recursive scope, debounce policy, and subscriber id;
- internal invalidation records when caching is introduced;
- deterministic path normalization across Windows, macOS, and Linux.

Model-visible tools are:

- `glob_files` for deterministic path-shape search;
- `grep_files` for literal content search.

Watcher and cache APIs should be kernel/runtime resources, not model-visible
tools.

## Crate Ownership

- `agent-os-sys`: traversal policy, watch subscription, file change event, and
  search result data types when they cross crate boundaries.
- `agent-os-kernel`: search tool execution, resource policy, deterministic
  result shaping, and replay-visible invalidation events.
- `agent-os-host` or a focused host service: OS watcher and platform IO backend
  if the kernel should not directly own long-lived OS handles.
- `agent-os-thread`: consumes `glob_files`, `grep_files`, and workspace context
  projections.
- `agent-os-conformance`: cross-platform path and search contract tests.

## Implementation Slices

1. Stabilize `glob_files` and `grep_files` as the only model-visible workspace
   discovery tools.
2. Remove `workspace_discovery` as a standalone long-term module if its logic can
   live cleanly inside the owning glob/grep driver; extract only a narrow
   traversal domain operation if tests prove the duplication is meaningful.
3. Keep the ignore policy as the canonical configured traversal contract:
   `.gitignore` switches ownership to configured rules, `.ignore` is read as
   part of that policy, and `target/` plus `node_modules/` are defaults only
   without `.gitignore`.
4. Add runtime cancellation handles for large workspaces while preserving
   deterministic bounded paging.
5. Add watch subscriptions and debounced change batches.
6. Wire watcher events to invalidate any internal search cache.
7. Add runtime/app-server projections for workspace changes.

## Validation

- Unit tests for glob semantics, literal search, ignore policy, gitignore
  policy, path normalization, symlink handling, binary skipping, and result
  paging.
- Integration tests over temporary workspaces for bounded traversal,
  cancellation, file creation/modification/deletion, and invalidation.
- Cross-platform tests for Windows separators, drive-root containment, Unix
  hidden files, and symlink loops.
- Runtime tests proving the model-visible discovery tools do not shell out and
  emit source-reference evidence.

## Forward-Only Notes

`glob_files` and `grep_files` should be the canonical permanent search surface.
Agent-OS should not keep old `search_files`, `workspace_discovery`, `find_files`,
and index-backed tools as parallel surfaces.
