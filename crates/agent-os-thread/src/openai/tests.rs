use super::*;
use crate::{ArtifactRecord, RuntimeConfig, RuntimeRunReport, ThreadRuntime};
use agent_os_kernel::{
    Kernel, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput, ToolInvokeInput,
};
use agent_os_store::LocalBlobStore;

#[path = "tests/live.rs"]
mod live;
#[path = "tests/mock_adapter.rs"]
mod mock_adapter;
#[path = "tests/support.rs"]
mod support;
#[path = "tests/unit.rs"]
mod unit;
