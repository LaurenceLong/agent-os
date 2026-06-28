//! Storage traits and deterministic local drivers.
//!
//! SQLite and PostgreSQL are later driver crates. The kernel depends on these
//! traits so storage technology never becomes kernel identity.

mod blob;
mod memory;
mod traits;

pub use blob::*;
pub use memory::*;
pub use traits::*;
