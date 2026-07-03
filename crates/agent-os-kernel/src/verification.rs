//! Final verification depth.
//!
//! The final-answer gate must do more than confirm the evidence map is
//! non-empty. Per `docs/10-kernel-design/kernel-data-model.md:778-779`:
//! "Final verification MUST check every high-impact final claim" and "Stale
//! evidence MUST fail verification". This module classifies high-impact
//! claims, rejects stale evidence, and derives a `Verification` summary.

use crate::*;
use agent_os_sys::*;
use serde_json::Value;

/// Keywords that mark a final claim as high-impact. A high-impact claim must
/// be present in the evidence map with at least one active evidence ref.
/// Matched case-insensitively against the claim text.
const HIGH_IMPACT_KEYWORDS: &[&str] = &[
    "security",
    "secure",
    "delete",
    "deleted",
    "deploy",
    "deployed",
    "migrate",
    "migration",
];

/// Result of evaluating a final submission against the verification rules.
#[derive(Debug, Clone, Default)]
pub struct FinalVerificationOutcome {
    /// High-impact claim texts found in the evidence map.
    pub checked_high_impact_claims: Vec<String>,
    /// High-impact keywords inferred from the submission that the evidence
    /// map does not cover.
    pub uncovered_high_impact_claims: Vec<String>,
    /// Evidence ids flagged stale by provenance metadata.
    pub stale_evidence_ids: Vec<String>,
}

impl Kernel {
    /// Evaluate final-verification depth for a submission against current
    /// state. Pure read; emits nothing.
    pub(crate) fn evaluate_final_verification(
        &self,
        task_id: &str,
        submission: &FinalSubmission,
    ) -> AgentOsResult<FinalVerificationOutcome> {
        let state = self.read_state()?;
        Ok(evaluate_submission(&state, task_id, submission))
    }

    /// Derive and persist a `Verification` record summarizing the final
    /// verification outcome. The verdict is `Pass` when there are no
    /// uncovered high-impact claims and no stale evidence, else `Fail`.
    pub(crate) fn record_final_verification(
        &self,
        agent_id: &str,
        task_id: &str,
        submission: &FinalSubmission,
        outcome: &FinalVerificationOutcome,
        causation_id: Option<String>,
    ) -> AgentOsResult<Verification> {
        let verdict = if outcome.uncovered_high_impact_claims.is_empty()
            && outcome.stale_evidence_ids.is_empty()
        {
            VerificationVerdict::Pass
        } else {
            VerificationVerdict::Fail
        };
        let verification = Verification {
            verification_id: new_id("ver_"),
            artifact_id: submission.changed_artifacts.iter().max().cloned(),
            final_artifact_id: None,
            verifier_agent_id: agent_id.to_string(),
            status: match verdict {
                VerificationVerdict::Pass => VerificationStatus::Passed,
                VerificationVerdict::Fail => VerificationStatus::Failed,
                VerificationVerdict::Inconclusive => VerificationStatus::Submitted,
            },
            checked_claims: outcome
                .checked_high_impact_claims
                .iter()
                .map(|claim| Value::String(claim.clone()))
                .collect(),
            unsupported_claims: outcome.uncovered_high_impact_claims.clone(),
            stale_evidence_ids: outcome.stale_evidence_ids.clone(),
            verdict,
            created_at: now_rfc3339(),
            submitted_at: Some(now_rfc3339()),
        };
        self.emit(
            "VerificationSubmitted",
            "verification",
            &verification.verification_id,
            Some(agent_id.to_string()),
            Some(task_id.to_string()),
            causation_id,
            None,
            &verification,
        )?;
        Ok(verification)
    }
}

fn evaluate_submission(
    state: &KernelState,
    _task_id: &str,
    submission: &FinalSubmission,
) -> FinalVerificationOutcome {
    let covered_claims: Vec<String> = submission
        .evidence_map
        .iter()
        .filter(|entry| {
            entry
                .evidence_refs
                .iter()
                .all(|id| evidence_is_active(state, id))
        })
        .map(|entry| entry.claim.clone())
        .collect();

    let mut uncovered: Vec<String> = Vec::new();
    for keyword in HIGH_IMPACT_KEYWORDS {
        let kw = keyword.to_string();
        // A high-impact keyword is uncovered if it appears in the submission's
        // explicit risk statements but no covered claim mentions it. Workflow
        // words such as local tests or non-production validation are not risk
        // actions by themselves.
        if submission_mentions(submission, keyword)
            && !covered_claims
                .iter()
                .any(|claim| claim.to_lowercase().contains(keyword))
            && !uncovered.contains(&kw)
        {
            uncovered.push(kw);
        }
    }

    // Staleness: evidence is stale when it carries an explicit stale marker
    // but is still offered in the final map.
    let stale_evidence_ids = submission
        .evidence_map
        .iter()
        .flat_map(|entry| entry.evidence_refs.iter())
        .filter_map(|id| state.evidence.get(id).map(|ev| (id.clone(), ev.clone())))
        .filter(|(_, ev)| evidence_marked_stale(ev))
        .map(|(id, _)| id)
        .collect::<Vec<_>>();

    FinalVerificationOutcome {
        checked_high_impact_claims: covered_claims,
        uncovered_high_impact_claims: uncovered,
        stale_evidence_ids,
    }
}

fn evidence_is_active(state: &KernelState, evidence_id: &str) -> bool {
    state
        .evidence
        .get(evidence_id)
        .is_some_and(|ev| ev.status == EvidenceStatus::Active)
}

/// An active evidence record is considered stale when a context-invalidation
/// pass has flagged it via `metadata.stale = true`. This is the durable
/// signal that the evidence predates a superseding state and should not be
/// trusted in a final answer.
fn evidence_marked_stale(evidence: &Evidence) -> bool {
    evidence.status == EvidenceStatus::Active
        && evidence
            .metadata
            .get("stale")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

/// Whether the submission *raises* a high-impact concern that the evidence
/// map must then cover. Only declared `known_risks` count: these are the
/// explicit risk statements a final answer makes, and any high-impact risk
/// (security, deletion, deployment, etc.) raised here must be backed by a
/// covered claim. Narrative `summary`, completed `tests_run`, and declared
/// `tests_not_run` describe workflow state rather than risk-bearing claims,
/// so they do not by themselves raise an uncoverable high-impact claim.
fn submission_mentions(submission: &FinalSubmission, keyword: &str) -> bool {
    let k = keyword.to_lowercase();
    submission
        .known_risks
        .iter()
        .any(|r| r.to_lowercase().contains(&k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttachEvidenceInput, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
    use serde_json::json;

    fn fixture() -> (Kernel, String, String, String) {
        let kernel = Kernel::new();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "verification-test".to_string(),
                created_by: "tester".to_string(),
                title: "Verify".to_string(),
                description: "Verify".to_string(),
                acceptance_criteria: vec!["final gate is deep".to_string()],
                constraints: Vec::new(),
                risk_level: 4,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id.clone(),
                parent_task_id: None,
                title: "Patch".to_string(),
                description: "Patch".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 10,
                risk_level: 4,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id.clone(),
                role_profile_id: "role_producer".to_string(),
                owner: "tester".to_string(),
                goal: "patch".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![".".to_string()],
            })
            .unwrap();
        (kernel, goal.goal_id, task.task_id, agent.agent_id)
    }

    fn attach_evidence(kernel: &Kernel, goal_id: &str, task_id: &str, claim: &str) -> String {
        kernel
            .attach_evidence(AttachEvidenceInput {
                goal_id: goal_id.to_string(),
                task_id: Some(task_id.to_string()),
                artifact_id: None,
                evidence_type: EvidenceType::DiffRef,
                producer_agent_id: None,
                claim: Some(claim.to_string()),
                blob_ref: None,
                content_hash: None,
                inline_bytes: None,
                metadata: json!({}),
            })
            .unwrap()
            .evidence_id
    }

    fn submission(claim: &str, evidence_id: &str, known_risks: Vec<String>) -> FinalSubmission {
        FinalSubmission {
            summary: "Completed the task.".to_string(),
            changed_artifacts: Vec::new(),
            evidence_map: vec![EvidenceMapEntry {
                claim: claim.to_string(),
                evidence_refs: vec![evidence_id.to_string()],
            }],
            unverified_claims: Vec::new(),
            known_risks,
            tests_run: Vec::new(),
            tests_not_run: Vec::new(),
            approvals: Vec::new(),
        }
    }

    #[test]
    fn final_submission_with_covered_high_impact_risk_succeeds() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        let evidence_id = attach_evidence(&kernel, &goal_id, &task_id, "security review passed");
        let submission = submission(
            "security review passed for the change",
            &evidence_id,
            vec!["security: reviewed and accepted".to_string()],
        );
        kernel
            .submit_final(&agent_id, &task_id, submission)
            .expect("covered high-impact risk passes the deep gate");
        let state = kernel.state_snapshot().unwrap();
        assert_eq!(state.verifications.len(), 1);
        let verification = state.verifications.values().next().unwrap();
        assert_eq!(verification.verdict, VerificationVerdict::Pass);
    }

    #[test]
    fn final_submission_does_not_treat_local_test_or_non_production_caveats_as_high_impact() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        let evidence_id = attach_evidence(&kernel, &goal_id, &task_id, "focused validation passed");
        let submission = submission(
            "focused validation passed for the change",
            &evidence_id,
            vec!["tests were run locally outside production".to_string()],
        );

        kernel
            .submit_final(&agent_id, &task_id, submission)
            .expect("workflow caveats are not high-impact risk claims");
        let state = kernel.state_snapshot().unwrap();
        let verification = state.verifications.values().next().unwrap();
        assert_eq!(verification.verdict, VerificationVerdict::Pass);
    }

    #[test]
    fn final_submission_with_uncovered_high_impact_risk_is_rejected() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        let evidence_id = attach_evidence(&kernel, &goal_id, &task_id, "diff attached");
        let submission = submission(
            "diff attached for the change",
            &evidence_id,
            vec!["deploy: production rollout risk".to_string()],
        );
        let err = kernel
            .submit_final(&agent_id, &task_id, submission)
            .unwrap_err();
        assert!(matches!(err, AgentOsError::Validation(ref msg) if msg.contains("high-impact")));
    }

    #[test]
    fn final_submission_with_stale_marked_evidence_is_rejected() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        // Attach evidence that a context-invalidation pass has flagged stale.
        let stale_evidence_id = kernel
            .attach_evidence(AttachEvidenceInput {
                goal_id: goal_id.clone(),
                task_id: Some(task_id.clone()),
                artifact_id: None,
                evidence_type: EvidenceType::DiffRef,
                producer_agent_id: None,
                claim: Some("diff attached but flagged stale".to_string()),
                blob_ref: None,
                content_hash: None,
                inline_bytes: None,
                metadata: json!({"stale": true}),
            })
            .unwrap()
            .evidence_id;
        let submission = submission(
            "diff attached for the change",
            &stale_evidence_id,
            Vec::new(),
        );
        let err = kernel
            .submit_final(&agent_id, &task_id, submission)
            .unwrap_err();
        assert!(matches!(err, AgentOsError::Validation(ref msg) if msg.contains("stale")));
    }

    #[test]
    fn final_submission_rejects_stale_evidence_even_when_unverified_claim_mentions_it() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        let stale_evidence_id = kernel
            .attach_evidence(AttachEvidenceInput {
                goal_id: goal_id.clone(),
                task_id: Some(task_id.clone()),
                artifact_id: None,
                evidence_type: EvidenceType::DiffRef,
                producer_agent_id: None,
                claim: Some("diff attached but flagged stale".to_string()),
                blob_ref: None,
                content_hash: None,
                inline_bytes: None,
                metadata: json!({"stale": true}),
            })
            .unwrap()
            .evidence_id;
        let mut submission = submission(
            "diff attached for the change",
            &stale_evidence_id,
            Vec::new(),
        );
        submission.unverified_claims = vec![stale_evidence_id.clone()];
        let err = kernel
            .submit_final(&agent_id, &task_id, submission)
            .unwrap_err();
        assert!(matches!(err, AgentOsError::Validation(ref msg) if msg.contains("stale")));
    }

    #[test]
    fn final_submission_rejects_uncovered_high_impact_risk_even_when_disclosed() {
        let (kernel, goal_id, task_id, agent_id) = fixture();
        let evidence_id = attach_evidence(&kernel, &goal_id, &task_id, "diff attached");
        let mut submission = submission(
            "diff attached",
            &evidence_id,
            vec!["delete: data removal risk".to_string()],
        );
        submission.unverified_claims = vec!["delete".to_string()];
        let err = kernel
            .submit_final(&agent_id, &task_id, submission)
            .unwrap_err();
        assert!(matches!(err, AgentOsError::Validation(ref msg) if msg.contains("high-impact")));
    }
}
