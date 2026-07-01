//! Agent-OS ABI and logical data contracts.
//!
//! This crate intentionally contains transport-safe Rust types rather than
//! kernel behavior. The kernel, store drivers, runtimes, and distributions all
//! share these contracts.

mod abi;
mod app;
mod artifacts;
mod audit;
mod automation;
mod communication;
mod context;
mod core;
mod ecosystem;
mod execution;
mod lifecycle;
mod memento;
mod package;
mod profiles;
mod provider;
mod resources;
mod tools;

pub use abi::*;
pub use app::*;
pub use artifacts::*;
pub use audit::*;
pub use automation::*;
pub use communication::*;
pub use context::*;
pub use core::*;
pub use ecosystem::*;
pub use execution::*;
pub use lifecycle::*;
pub use memento::*;
pub use package::*;
pub use profiles::*;
pub use provider::*;
pub use resources::*;
pub use tools::*;
