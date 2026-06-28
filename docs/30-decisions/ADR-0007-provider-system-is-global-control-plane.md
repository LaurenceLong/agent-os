# ADR-0007: Provider System is global control-plane infrastructure

Status: accepted

Date: 2026-06-25

## Context

Agent-OS needs a unified way to obtain LLM streams. If provider handling is left inside Agent Thread Runtime or scattered across distributions, the system will drift into per-agent SDK glue, duplicated secrets logic, inconsistent routing, and untestable override behavior.

The project needs something closer to a system-wide `cc-switch`: one unified provider configuration and routing layer that every Agent Thread uses.

## Decision

Agent-OS will introduce a system-level Provider System.

The Provider System will own:

- provider profiles
- model aliases
- model catalog
- routing policies
- credential resolution
- fallback policy
- normalized stream sessions
- usage and cost accounting
- provider adapter plugins

Agent Threads will obtain LLM streams only through the Provider System. They will not talk directly to provider SDKs.

## Consequences

Positive:

- provider behavior becomes consistent across all agents and distributions
- model routing and fallback become testable
- credentials and quotas can be managed centrally
- thread runtime stays provider-neutral
- changing providers or model aliases does not require thread logic rewrites

Negative:

- another kernel-adjacent subsystem must be built early
- profile and routing configuration become first-class design work
- local development needs a good default config story

## Required Follow-Up

The architecture docs MUST treat `Model Gateway` as a facade inside the Provider System, not as the whole provider design.

The first implementation milestones MUST include provider profile resolution, model aliasing, normalized stream events, and fallback event emission.

