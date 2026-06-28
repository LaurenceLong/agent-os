//! SQLite storage driver for the single-node Agent-OS kernel.
//!
//! The driver stores the durable append-only event log and idempotent syscall
//! results. Kernel projections remain rebuildable from `events`.

mod error;
mod events;
mod idempotency;
mod migrations;
mod projection;
mod store;

pub use store::SqliteStore;
