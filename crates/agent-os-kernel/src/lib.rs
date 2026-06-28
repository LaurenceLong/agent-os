//! Single-node Agent-OS microkernel.
//!
//! This crate implements the first contract-driven kernel spine: profile
//! resolution, ATCB lifecycle, syscall admission, append-only events,
//! deterministic replay, communication gates, resource/budget arbitration,
//! memento immutability, artifacts, evidence, review, verification, and final
//! answer gates.

mod artifacts;
mod blackboard;
mod capability;
mod communication;
mod context;
mod events;
mod export;
mod goals;
mod inputs;
mod memento;
mod profile_seed;
mod profiles;
mod provider;
mod resources;
mod review;
mod schema;
mod state;
mod syscall;
mod tasks;
mod threads;
mod tools;
mod util;

pub use export::*;
pub use inputs::*;
pub use state::{Kernel, KernelState};
