//! Agent-OS distribution prompt and workflow package support.
//!
//! Distribution crates own packaged prompts, workflow policies, and goal-level
//! prompt material. Runtime crates consume prepared inputs and do not own these
//! distribution rules.

mod distro;
mod planner;
mod types;
mod workflow;

pub use types::{
    SoftwareCodeTask, SoftwareEditPlanSource, SoftwareExactEdit, SoftwareWorkflowPrompt,
    SoftwareWorkflowRequest, SoftwareWorkflowStep,
};
