//! Single-node Agent-OS microkernel.
//!
//! This crate implements the first contract-driven kernel spine: profile
//! resolution, ATCB lifecycle, syscall admission, append-only events,
//! deterministic replay, communication gates, resource/budget arbitration,
//! memento immutability, artifacts, evidence, review, verification, and final
//! answer gates.

mod artifacts;
mod automation;
mod blackboard;
mod capability;
mod communication;
mod context;
mod ecosystem;
mod events;
mod export;
mod goals;
mod inputs;
mod memento;
mod packages;
mod permissions;
mod process;
mod profile_seed;
mod profiles;
mod provider;
mod recovery;
mod resources;
mod review;
mod scheduler;
mod schema;
mod state;
mod syscall;
mod tasks;
mod threads;
mod tools;
mod util;
mod verification;

pub use ecosystem::{
    discover_mcp_resource_definitions, discover_mcp_resource_template_definitions,
    discover_mcp_tool_definitions, mcp_tool_descriptor,
};
pub use export::*;
pub use inputs::*;
pub use recovery::ReconciliationReport;
pub use scheduler::{AdmissionDecision, AdmissionRejection};
pub use state::{Kernel, KernelState};
