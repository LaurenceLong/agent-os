# ADR-0002: PostgreSQL is a storage driver, not kernel state

Status: accepted

Date: 2026-06-25

## Context

Agent-OS needs durable state for Agent Control Blocks, task DAGs, typed blackboards, evidence, artifacts, audit logs, and replay.

PostgreSQL is a strong production choice for structured control-plane data. However, treating PostgreSQL as part of the kernel would bind the operating model to database operations, deployment topology, connection management, and HA strategy.

## Decision

PostgreSQL is an official production storage driver.

It is not part of the kernel identity.

The kernel depends on storage traits and schemas:

- EventStore
- ProjectionStore
- LockStore
- LeaseStore
- MemoryStore
- AuditStore
- ArtifactBlobStore
- EvidenceBlobStore

SQLite is the default local driver. PostgreSQL is the production control-plane driver.

## Consequences

Positive:

- local development remains simple
- production deployment remains strong
- kernel contracts stay portable
- future storage drivers remain possible

Negative:

- storage abstraction must be carefully tested
- drivers must pass shared conformance suites
- some PostgreSQL-specific optimizations may stay outside the portable core

## Implications

Code in kernel crates MUST NOT call PostgreSQL-specific APIs directly.

