//! Agent Thread Runtime.
//!
//! This crate owns the runtime loop around kernel-managed Agent Threads. The
//! kernel remains the authority for turns, providers, tools, evidence,
//! artifacts, and final submission gates; this crate consumes model actions and
//! turns them into kernel syscalls and structured runtime state.

mod external;
mod handle;
mod model;
mod openai;
mod ops;
mod runtime;
mod scripted;
mod software;
mod types;

pub use external::ExternalProcessModelClient;
pub use handle::AgentThreadHandle;
pub use model::{
    ArtifactRecord, ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, ToolAction,
    ToolExecutionRecord,
};
pub use openai::{LlmApiStyle, OpenAiModelClient};
pub use ops::mock_turn_start_op;
pub use runtime::{RuntimeConfig, RuntimeRunOverrides, RuntimeRunReport, ThreadRuntime};
pub use scripted::{ScriptedModelClient, ScriptedStep};
pub use software::{
    ReviewRevision, SoftwareCodeTask, SoftwareEditPlanSource, SoftwareEngineeringPipeline,
    SoftwarePipelineReport, SoftwareReplaySummary,
};
pub use types::{AgentOpAck, ThreadStatusSnapshot, TurnStartAck};
