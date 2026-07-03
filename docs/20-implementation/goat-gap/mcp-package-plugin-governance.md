# MCP, Package, And Plugin Governance Plan

Status: planning

Last updated: 2026-07-03

## Goal

Turn skills, MCP servers, packages, plugins, connectors, and installable tool
capabilities into governed Agent-OS resources with deterministic import,
enablement, discovery, execution, audit, and replay behavior.

## Non-Goals

- Do not keep one-off MCP launch as the long-term execution path.
- Do not let package install state mutate outside kernel-visible records.
- Do not make marketplace recommendations equivalent to installed tools.
- Do not add broad plugin power before capability boundaries are typed.

## Current Agent-OS State

Agent-OS imports ecosystem sources into kernel state and has typed records for
skills, MCP server specs, MCP tool definitions, MCP resource metadata, MCP
resource-template metadata, package install records, package contribution
records, and tool descriptors. It can discover local stdio MCP tools/resources
and execute an MCP tool through the tool broker.

The gap is governance:

- MCP calls spawn local stdio servers per operation instead of using a managed
  connection lifecycle;
- transport is local stdio only;
- package/plugin manifests are still narrow package contracts rather than a full
  plugin app/hook/interface contract;
- package install/enable/disable and contribution state are modeled, but
  share/cache/version lifecycle policy is still incomplete;
- connector and marketplace suggestion flows do not exist;
- MCP auth, OAuth, elicitation, resource content reads, and disabled-tool policy
  are not full kernel resources.

## Codex Reference

Codex has plugin manifests for skills, MCP servers, hooks, apps, and interface
metadata. Core plugin management tracks configured, installed, curated,
non-curated, remote, enabled, disabled, marketplace, cache, and recommendation
state. MCP uses a connection manager with cached server metadata, resource and
tool operations, disabled-tool checks, startup cancellation, and RMCP transport
support for local stdio, executor stdio, streamable HTTP retry, OAuth, and auth
status.

## Target Agent-OS Contract

Agent-OS should define:

- `AgentPackageManifest`: package identity, version, source, capabilities,
  declared skills, MCP servers, profile seeds, prompts, hooks, and app
  contributions.
- `PackageInstallRecord`: installed source, content hash, enabled state,
  trust/auth policy, install provenance, and cache metadata.
- `PluginContribution`: normalized kernel import output from a package.
- `McpServerRuntime`: transport, auth state, startup state, cached capabilities,
  disabled tools, resources, resource templates, and connection health.
- `InstallCandidate`: discoverable but not installed package or connector,
  separate from active tools.

MCP tool descriptors and resource metadata should be projections of active
`McpServerRuntime` state, not direct output from a one-shot discovery command.
The current implementation has typed server/tool/resource/resource-template
metadata and replayable registration events; the managed runtime remains a
future slice.

## Crate Ownership

- `agent-os-sys`: package, plugin, connector, MCP transport, auth, and install
  candidate data types.
- `agent-os-kernel`: package registry, enable/disable policy, import events,
  MCP runtime state, connection lifecycle, tool projection, permission checks.
- `agent-os-ecosystem`: filesystem/package discovery and manifest parsing before
  kernel import.
- `agent-os-thread`: model-visible tool projection only.
- `agent-os-cli`: package and plugin commands as orchestration wrappers.

## Implementation Slices

1. Replace broad ecosystem source kinds with package/plugin identities where the
   current contract is clearer.
2. Add package manifest parsing and validation. Implemented.
3. Add package install/enable/disable events and kernel projections.
   Implemented for local package manifest records.
4. Move skills and MCP server imports under package/plugin contribution records.
   Implemented for instructions, skills, commands, imported agents, MCP servers,
   MCP tools, MCP resources, and MCP resource templates.
5. Add persistent MCP connection management for local stdio.
6. Add MCP resource/resource-template listing through kernel APIs. Implemented
   for typed metadata discovery, registration, package contribution, runtime
   projection, app projection, and replay.
7. Add disabled-tool and per-server policy.
8. Add install-candidate discovery and governed install request tools.
9. Extend transport to streamable HTTP and auth/OAuth only after the local stdio
   lifecycle is deterministic.

## Validation

- Unit tests for manifest parsing, invalid manifests, capability normalization,
  source containment, and package identity collisions.
- Kernel integration tests for install, enable, disable, import replay, and
  deterministic tool projections.
- MCP integration tests for local stdio startup, tools/list, tools/call,
  resources/list, resource templates, disabled tools, denial, timeout, and
  connection recovery.
- Runtime tests for deferred MCP/plugin tool discovery.
- Ignored live LLM e2e tests for loading a plugin-provided skill, selecting an
  MCP tool, handling denial, and submitting final evidence.

## Forward-Only Notes

The current local stdio MCP path should become an implementation backend under
`McpServerRuntime`. It should not remain as a second canonical tool execution
path.
