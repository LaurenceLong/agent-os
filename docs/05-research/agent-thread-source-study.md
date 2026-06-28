# Agent Thread Source Study

Status: research input

Last updated: 2026-06-25

## 1. Scope and Source Boundary

This study informs the first Agent-OS core module: Agent Thread.

Sources used:

- OpenCode public source from [anomalyco/opencode](https://github.com/anomalyco/opencode). The local study copy was `D:\work\ai_agents\coding-agent\opencode-dev`; it was not a git checkout at review time, so no commit hash is available from this repository. Future source studies MUST record commit SHA or release tag before treating observations as reproducible.
- OpenAI Codex public source cloned from [openai/codex](https://github.com/openai/codex), local path `D:\work\ai_agents\coding-agent\codex-openai`, commit `6368937939dceb07b4a3c47c4448027d0d1a85a6` (`2026-06-25T10:10:36+01:00`, "Support HTTP MCP servers from selected executor plugins (#28522)").
- Public Claude Code documentation:
  - [Subagents](https://code.claude.com/docs/en/sub-agents)
  - [Permissions](https://code.claude.com/docs/en/permissions)
  - [Hooks](https://code.claude.com/docs/en/hooks)
  - [Auto mode configuration](https://code.claude.com/docs/en/auto-mode-config)
  - [Sandboxing](https://code.claude.com/docs/en/sandboxing)
  - Anthropic engineering note: [Claude Code auto mode](https://www.anthropic.com/engineering/claude-code-auto-mode)

Non-public or leaked Claude Code source was not inspected and MUST NOT become an implementation dependency. The project can learn from public behavior and documented interfaces, but the Agent-OS kernel must remain a clean-room design.

## 2. Research Goal

The goal is not to copy an existing coding agent.

The goal is to extract production control-plane patterns that are already battle-tested:

- typed thread and turn boundaries
- event streams
- tool lifecycle state
- permission and sandbox boundaries
- provider abstraction
- subagent isolation
- lifecycle hooks
- context compaction
- recovery and replay

Agent-OS must then harden these patterns into kernel contracts.

## 3. OpenCode Strengths

### 3.1 Provider Abstraction

OpenCode normalizes providers into a language model interface and keeps provider quirks in a transform layer. It also uses provider and model metadata to decide which tools or features are visible.

Agent-OS should adopt the pattern, but the provider layer must be a kernel driver boundary and a system-level control-plane module. The Agent Thread Runtime must not be tied to one SDK or one provider API.

### 3.2 Typed Message Parts

OpenCode models assistant output as typed parts such as text, reasoning, files, tool calls, step starts, step finishes, snapshots, patches, compaction, retry, agent messages, and subtasks.

Agent-OS should adopt typed Agent Items. Chat history must be a derived projection, not the source of truth.

### 3.3 Stream Processor

OpenCode's session processor handles model stream events, tool-call start/result/error, reasoning deltas, text deltas, step finish, usage, cost, patch capture, compaction triggers, and cleanup for incomplete items.

Agent-OS should make this a formal Agent Turn state machine with explicit state transitions and durable events.

### 3.4 Permission Rules

OpenCode supports allow, deny, and ask rules, stores pending approval requests, persists accepted rules, and can hide disabled tools before the model sees them.

Agent-OS should adopt:

- tool visibility filtering before model call
- execution-time permission decisions
- persistent scoped approval records
- denied-call audit events

Agent-OS should harden this by making permissions capability-based and task-bound.

### 3.5 Tool Registry

OpenCode validates tool input schemas, formats validation errors, truncates oversized output, supports built-in and plugin tools, and can vary tool visibility by provider or model.

Agent-OS should adopt schema-first tool invocation and output normalization, but every invocation must pass through Tool Broker as a syscall.

### 3.6 Operational Guards

OpenCode captures snapshots and patches around model/tool steps, detects repeated tool-call loops, and triggers compaction when context becomes unsafe.

Agent-OS should adopt snapshot, patch, loop, and compaction events as first-class kernel signals.

### 3.7 Limits

OpenCode agents are useful configuration overlays, but they are not kernel threads. They do not define an OS-level ABI, Agent Control Block, artifact lifecycle, evidence map, or independent verification contract.

## 4. OpenAI Codex Strengths

### 4.1 Rust Core and Protocol Boundary

Codex separates core runtime, app-server protocol, thread store, agent graph store, model provider, exec policy, sandboxing, and tool crates.

Agent-OS should adopt this separation. Kernel crates should be Rust-first, while SDKs and distributions can use TypeScript or Python.

### 4.2 Thread and Turn Boundary

Codex models a long-lived thread separately from individual turns. A thread exposes operations such as submit, steer, interrupt, inject, status subscription, background terminal management, rollout flushing, and configuration snapshots.

Agent-OS should define:

- Agent Thread as the durable execution object
- Agent Turn as one active model/tool loop
- Agent Step as one model request plus its tool batch and post-step accounting

### 4.3 Submission Queue and Event Queue

Codex uses a submission queue for operations and an event queue for lifecycle events.

Agent-OS should adopt this as the Agent Thread IPC baseline:

- all external control enters through Agent Ops
- all observable state leaves through Agent Events
- all durable state is rebuilt from events and snapshots

### 4.4 Turn-Scoped Model Session

Codex distinguishes session-scoped model clients from turn-scoped request state. Turn routing state is kept per turn and must not leak across turns.

Agent-OS should require a Model Turn Session for every Agent Turn. Provider sticky state, request headers, cache tokens, and retry metadata must be scoped to that turn unless explicitly persisted by the kernel.

### 4.5 Configuration Snapshot

Codex snapshots model, provider, service tier, approval policy, permission profile, sandbox policy, workspace roots, environment selection, reasoning settings, collaboration mode, personality, session source, and parent/fork metadata.

Agent-OS should make a Thread Config Snapshot mandatory at every turn start.

### 4.6 Agent Control and Registry

Codex has an Agent Control layer that spawns agents, sends inter-agent communication, tracks live agents, enforces execution capacity, reserves spawn slots, assigns paths and nicknames, persists spawn edges, and can unload resident subagents.

Agent-OS should adopt:

- Agent Registry
- spawn reservations
- max active Agent Thread limits
- agent tree paths
- parent-child edges
- resident/unloaded states
- completion watchers

Agent-OS should harden this with resource locks, artifact ownership, and role-specific capability grants.

### 4.7 Typed App Protocol

Codex exposes typed protocol objects for threads, turns, items, permissions, command execution, reviews, token usage, background terminals, and settings updates.

Agent-OS should define a stable ABI and generated SDKs from the beginning.

### 4.8 Tool Lifecycle and Extensions

Codex routes model tool calls through a Tool Router and Registry, supports model-visible specs, dynamic tools, MCP, shell/apply-patch runtimes, cancellation, parallelism metadata, lifecycle contributors, and turn-level diff tracking.

Agent-OS should adopt tool lifecycle contributors as kernel hooks, but hooks must not bypass capabilities.

### 4.9 Limits

Codex is a coding agent runtime, not a general Agent-OS kernel. It does not define a Linux-like ecosystem where third-party distributions build on a stable kernel ABI. Agent-OS must generalize the thread, tool, permission, evidence, and artifact contracts beyond coding.

## 5. Public Claude Code Behavior Strengths

This section uses public documentation only.

### 5.1 Subagent Isolation

Claude Code supports subagents with their own model selection, tool availability, permission behavior, skills, memory scope, foreground/background execution, and optional worktree isolation.

Agent-OS should adopt isolation as a kernel-managed resource:

- context isolation
- permission isolation
- memory namespace isolation
- workspace/worktree isolation
- process and sandbox isolation

### 5.2 Permission Precedence

Public Claude Code documentation states that permission rules are evaluated with deny before ask before allow, and that permissions are enforced by the runtime rather than by model instructions.

Agent-OS should adopt deny-first semantics. A model prompt can request behavior, but it can never grant itself access.

### 5.3 Hooks

Claude Code exposes lifecycle hooks such as PreToolUse, PostToolUse, PostToolBatch, SubagentStart, SubagentStop, PreCompact, PostCompact, WorktreeCreate, WorktreeRemove, PermissionRequest, and PermissionDenied.

Agent-OS should adopt a hook bus, but with stricter semantics:

- hooks may deny, add context, request stricter review, or attach evidence
- hooks may not grant capabilities the kernel did not grant
- hooks may not mutate kernel state except through syscalls
- hook output must be auditable

### 5.4 Auto Mode Classifier

Claude Code auto mode routes some permission decisions through a classifier, while deny and explicit ask rules remain stronger gates.

Agent-OS should treat classifier approval as a policy assistant, not as authority. Hard policy, capabilities, role restrictions, and human approvals must take precedence.

### 5.5 Sandboxed Bash

Claude Code documents sandboxed command execution with auto-allow behavior inside sandbox boundaries and regular permission flow when a command cannot be safely sandboxed.

Agent-OS should adopt sandbox-aware approval reduction, but sandbox unavailability must be configurable as a hard failure for production distributions.

### 5.6 Limits

Public Claude Code behavior is product behavior, not an Agent-OS kernel contract. Its subagents, hooks, and permission modes are useful reference points, but Agent-OS must define its own ABI and state machine.

## 6. Design Intake

| Observed pattern | Agent-OS adoption | Hardening beyond source |
|---|---|---|
| Typed message parts | Agent Items | Durable event schema and replay |
| Thread and turn split | Agent Thread, Turn, Step | Kernel-owned ACB and lifecycle validation |
| Tool registry | Tool Broker | Syscall envelope, capability tokens, evidence |
| Allow/ask/deny permissions | Permission Kernel | Deny-first, task-bound, role-bound, auditable |
| Provider transforms | Provider System adapters | Model capability catalog, routing policy, and ABI tests |
| Snapshots and patches | Step snapshots and artifact diffs | Evidence-linked artifact lifecycle |
| Spawn registry | Agent Registry | reservations, limits, tree paths, resource locks |
| Subagent worktree isolation | Workspace Isolation Manager | deterministic cleanup and artifact export |
| Hooks | Lifecycle Policy Hook Bus | hooks cannot grant authority |
| Auto approval classifier | Risk Classifier driver | advisory only, never above hard policy |
| Context compaction | ContextCompaction item | explicit provenance and replacement history |
| Background processes | Background Tool Process Manager | lease, heartbeat, kill, and audit semantics |

## 7. Non-Adoptions

Agent-OS will not adopt:

- an existing open-source agent loop as the kernel
- chat transcript as canonical state
- prompt-defined permissions
- untyped tool calls
- model-specific provider APIs in the Agent Thread core
- hooks that can bypass capability checks
- classifier approval as a production authority
- leaked or non-public source as a design dependency

## 8. Requirements Derived From Research

The Agent Thread Core Module must provide:

1. A durable Agent Thread object with a kernel-owned Agent Thread Control Block.
2. A thread/turn/step/item object model.
3. Submission Queue and Event Queue semantics.
4. Turn-scoped model sessions.
5. Provider-neutral model streaming.
6. System-level provider profiles, model aliases, and routing policies.
7. Typed Agent Items and model-visible projections.
8. Tool lifecycle states and evidence attachment.
9. Deny-first capability and permission checks.
10. Lifecycle policy hooks that can restrict but not grant authority.
11. Spawn reservations, hierarchy tracking, capacity limits, and resident unloading.
12. Workspace, process, permission, memory, and context isolation.
13. Crash recovery through events, checkpoints, and replay.
14. Conformance tests that make third-party distributions possible.
