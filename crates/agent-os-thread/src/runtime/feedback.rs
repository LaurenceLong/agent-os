use crate::{ArtifactRecord, ToolAction, ToolExecutionRecord};
use agent_os_sys::{new_id, ToolCallStatus};
use serde_json::{json, Value};

pub(super) const RUNTIME_FEEDBACK_TOOL: &str = "runtime_feedback";
pub(super) const MAX_CONSECUTIVE_NO_ACTION_TURNS: u32 = 2;
pub(super) const DUPLICATE_TOOL_WARNING_COUNT: u32 = 2;
pub(super) const MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS: u32 = 5;
const PRE_PATCH_FEEDBACK_TOOL_RESULTS: usize = 16;
pub(super) const PRE_PATCH_HARD_GATE_TOOL_RESULTS: usize = 24;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct RepeatedToolCallTracker {
    last: Option<ToolActionFingerprint>,
    count: u32,
}

impl RepeatedToolCallTracker {
    pub(super) fn observe(&mut self, action: &ToolAction) -> u32 {
        let fingerprint = ToolActionFingerprint::from(action);
        if self.last.as_ref() == Some(&fingerprint) {
            self.count += 1;
        } else {
            self.last = Some(fingerprint);
            self.count = 1;
        }
        self.count
    }

    pub(super) fn reset(&mut self) {
        self.last = None;
        self.count = 0;
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ToolActionFingerprint {
    tool_name: String,
    input: Value,
}

impl From<&ToolAction> for ToolActionFingerprint {
    fn from(action: &ToolAction) -> Self {
        Self {
            tool_name: action.tool_name.clone(),
            input: action.input.clone(),
        }
    }
}

pub(super) fn should_guard_duplicate_tool_call(action: &ToolAction) -> bool {
    !action.tool_name.is_empty()
}

pub(super) fn duplicate_tool_feedback_record(
    step_index: u32,
    consecutive_identical_tool_calls: u32,
    action: &ToolAction,
) -> ToolExecutionRecord {
    let is_blocking = consecutive_identical_tool_calls >= MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS;
    let severity = if is_blocking { "error" } else { "warning" };
    let message = if is_blocking {
        format!(
            "The model repeated an identical tool call {consecutive_identical_tool_calls} consecutive times. The runtime is blocking the task because the previous identical result was already available and the model did not choose a different action."
        )
    } else {
        format!(
            "Warning: the model repeated an identical tool call {consecutive_identical_tool_calls} consecutive times. The runtime rejected this duplicate without executing it because the previous identical result is already available in context. Choose a different focused tool call, apply the fix, or call submit_final if the task is complete or blocked with evidence."
        )
    };
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "consecutive_identical_tool_calls": consecutive_identical_tool_calls,
            "tool_name": action.tool_name,
            "tool_input": action.input,
        })),
        output: Some(json!({
            "message": message,
            "severity": severity,
            "duplicate_tool_warning_count": DUPLICATE_TOOL_WARNING_COUNT,
            "max_consecutive_identical_tool_calls": MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

pub(super) fn should_project_finalization_feedback(
    tool_results: &[ToolExecutionRecord],
    artifacts: &[ArtifactRecord],
) -> bool {
    if artifacts.is_empty() {
        return false;
    }
    latest_patch_has_following_command_evidence(tool_results)
}

fn latest_patch_has_following_command_evidence(tool_results: &[ToolExecutionRecord]) -> bool {
    let Some(last_patch_index) = tool_results.iter().rposition(|result| {
        result.tool_name == "apply_patch" && result.status == ToolCallStatus::Completed
    }) else {
        return false;
    };
    tool_results
        .iter()
        .skip(last_patch_index + 1)
        .any(|result| {
            result.tool_name == "run_command"
                && result.status == ToolCallStatus::Completed
                && !result.evidence_ids.is_empty()
        })
}

pub(super) fn finalization_feedback_record(
    step_index: u32,
    remaining_steps: u32,
    artifact_count: usize,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "remaining_steps": remaining_steps,
            "artifact_count": artifact_count,
        })),
        output: Some(json!({
            "message": "A patch plus command evidence already exist. On the next turn, call submit_final if the patch is complete or blocked by captured environment evidence. If the local goal must be closed first, call accomplish_goal and then submit_final. Do not call more workspace inspection, command, or patch tools after this feedback.",
            "remaining_steps": remaining_steps,
            "artifact_count": artifact_count,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

fn is_finalization_allowed_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, "submit_final" | "accomplish_goal")
}

pub(super) fn is_finalization_allowed_tool_call(action: &ToolAction) -> bool {
    is_finalization_allowed_tool_name(&action.tool_name)
}

pub(super) fn finalization_gate_feedback_record(
    step_index: u32,
    action: &ToolAction,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "rejected_tool_name": action.tool_name,
            "rejected_tool_input": action.input,
        })),
        output: Some(json!({
            "message": "The finalization gate is active. The runtime did not execute another exploratory tool call because a patch plus post-patch command evidence already exist. Call submit_final now. If the local goal must be closed first, call accomplish_goal and then submit_final.",
            "allowed_next_actions": ["submit_final", "accomplish_goal"],
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

pub(super) fn should_project_pre_patch_resolution_feedback(
    tool_results: &[ToolExecutionRecord],
    artifacts: &[ArtifactRecord],
) -> bool {
    is_pre_patch_resolution_gate_active(tool_results, artifacts)
        && count_pre_patch_investigation_tool_results(tool_results)
            >= PRE_PATCH_FEEDBACK_TOOL_RESULTS
}

pub(super) fn should_enforce_pre_patch_resolution_gate(
    tool_results: &[ToolExecutionRecord],
    artifacts: &[ArtifactRecord],
) -> bool {
    is_pre_patch_resolution_gate_active(tool_results, artifacts)
        && count_pre_patch_investigation_tool_results(tool_results)
            >= PRE_PATCH_HARD_GATE_TOOL_RESULTS
}

fn is_pre_patch_resolution_gate_active(
    tool_results: &[ToolExecutionRecord],
    artifacts: &[ArtifactRecord],
) -> bool {
    artifacts.is_empty() && !has_apply_patch_attempt(tool_results)
}

fn has_apply_patch_attempt(tool_results: &[ToolExecutionRecord]) -> bool {
    tool_results
        .iter()
        .any(|result| result.tool_name == "apply_patch")
}

pub(super) fn count_pre_patch_investigation_tool_results(
    tool_results: &[ToolExecutionRecord],
) -> usize {
    tool_results
        .iter()
        .filter(|result| {
            result.status == ToolCallStatus::Completed
                && matches!(
                    result.tool_name.as_str(),
                    "read_file" | "run_command" | "load_skill" | "read_skill_resource"
                )
        })
        .count()
}

pub(super) fn pre_patch_resolution_feedback_record(
    step_index: u32,
    investigation_tool_results: usize,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "investigation_tool_results": investigation_tool_results,
            "pre_patch_feedback_tool_results": PRE_PATCH_FEEDBACK_TOOL_RESULTS,
            "pre_patch_hard_gate_tool_results": PRE_PATCH_HARD_GATE_TOOL_RESULTS,
        })),
        output: Some(json!({
            "message": "The pre-patch investigation budget is nearly exhausted. Prefer calling apply_patch with the smallest scoped edit if enough evidence exists, or submit_final with blocker evidence. Continued investigation remains available for a short window, but the runtime will soon narrow the tool surface until at least one apply_patch attempt has been made.",
            "preferred_next_actions": ["apply_patch", "submit_final", "accomplish_goal"],
            "investigation_tool_results": investigation_tool_results,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

fn is_pre_patch_resolution_allowed_tool_name(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "apply_patch" | "submit_final" | "accomplish_goal"
    )
}

pub(super) fn is_pre_patch_resolution_allowed_tool_call(action: &ToolAction) -> bool {
    is_pre_patch_resolution_allowed_tool_name(&action.tool_name)
}

pub(super) fn pre_patch_resolution_gate_feedback_record(
    step_index: u32,
    action: &ToolAction,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "rejected_tool_name": action.tool_name,
            "rejected_tool_input": action.input,
        })),
        output: Some(json!({
            "message": "The pre-patch resolution gate is active. The runtime did not execute another investigation tool because the pre-patch investigation budget is exhausted. Call apply_patch now, or submit_final with blocker evidence.",
            "allowed_next_actions": ["apply_patch", "submit_final", "accomplish_goal"],
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}

pub(super) fn unsupported_image_input_tool_record(
    step_index: u32,
    action: &ToolAction,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: action.tool_name.clone(),
        status: ToolCallStatus::Failed,
        input: Some(action.input.clone()),
        output: Some(json!({
            "status": "failed",
            "stage": "model_capability",
            "error": "read_image requires a model with image_input capability",
            "step_index": step_index,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: action.evidence_claim.clone(),
    }
}

pub(super) fn runtime_feedback_record(
    step_index: u32,
    consecutive_no_action_turns: u32,
    output_texts: &[String],
) -> ToolExecutionRecord {
    let text = output_texts.join("\n\n");
    let text_excerpt = text.chars().take(1200).collect::<String>();
    ToolExecutionRecord {
        call_id: new_id("feedback_"),
        tool_name: RUNTIME_FEEDBACK_TOOL.to_string(),
        status: ToolCallStatus::Failed,
        input: Some(json!({
            "step_index": step_index,
            "consecutive_no_action_turns": consecutive_no_action_turns
        })),
        output: Some(json!({
            "message": "The previous model response had no tool call or final submission. On the next turn, call exactly one available tool or call submit_final if the task is complete or blocked with evidence.",
            "max_consecutive_no_action_turns": MAX_CONSECUTIVE_NO_ACTION_TURNS,
            "text_excerpt": text_excerpt,
        })),
        evidence_ids: Vec::new(),
        evidence_claim: None,
    }
}
