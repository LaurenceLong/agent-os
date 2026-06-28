use crate::{
    ArtifactRecord, ModelAction, ModelClient, ModelTurnRequest, ModelTurnResponse, ToolAction,
};
use agent_os_sys::*;
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum ScriptedStep {
    OutputText(String),
    ToolCall(ToolAction),
    Final {
        summary: String,
        known_risks: Vec<String>,
        tests_run: Vec<String>,
        tests_not_run: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ScriptedModelClient {
    steps: VecDeque<ScriptedStep>,
}

impl ScriptedModelClient {
    pub fn new(steps: impl IntoIterator<Item = ScriptedStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

impl ModelClient for ScriptedModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        let step = self.steps.pop_front().ok_or_else(|| {
            AgentOsError::Validation("scripted model exhausted before final submission".to_string())
        })?;
        let action = match step {
            ScriptedStep::OutputText(text) => ModelAction::OutputText { text },
            ScriptedStep::ToolCall(action) => ModelAction::ToolCall(action),
            ScriptedStep::Final {
                summary,
                known_risks,
                tests_run,
                tests_not_run,
            } => ModelAction::Final {
                submission: scripted_final_submission(
                    summary,
                    known_risks,
                    tests_run,
                    tests_not_run,
                    &request.artifacts,
                    &request.tool_results,
                ),
            },
        };
        Ok(ModelTurnResponse {
            actions: vec![action],
            usage: ProviderUsage {
                input_tokens: request.thread.task.local_goal.len() as u64,
                output_tokens: 1,
                cost: 0.0,
            },
        })
    }
}

fn scripted_final_submission(
    summary: String,
    known_risks: Vec<String>,
    tests_run: Vec<String>,
    tests_not_run: Vec<String>,
    artifacts: &[ArtifactRecord],
    tool_results: &[crate::ToolExecutionRecord],
) -> FinalSubmission {
    let evidence_map = tool_results
        .iter()
        .filter(|result| !result.evidence_ids.is_empty())
        .map(|result| EvidenceMapEntry {
            claim: result
                .evidence_claim
                .clone()
                .unwrap_or_else(|| format!("tool {} completed with evidence", result.tool_name)),
            evidence_refs: result.evidence_ids.clone(),
        })
        .collect();
    FinalSubmission {
        summary,
        changed_artifacts: artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        evidence_map,
        unverified_claims: Vec::new(),
        known_risks,
        tests_run,
        tests_not_run,
        approvals: Vec::new(),
    }
}

impl From<ToolAction> for ScriptedStep {
    fn from(value: ToolAction) -> Self {
        Self::ToolCall(value)
    }
}

impl From<Value> for ScriptedStep {
    fn from(value: Value) -> Self {
        Self::OutputText(value.to_string())
    }
}
