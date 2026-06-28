use super::BundleSelection;
use crate::KernelState;

pub(super) fn collect_artifacts(state: &KernelState, selection: &mut BundleSelection) {
    for artifact in state.artifacts.values() {
        if selection.task_ids.contains(&artifact.task_id)
            || selection.artifact_ids.contains(&artifact.artifact_id)
        {
            selection.artifact_ids.insert(artifact.artifact_id.clone());
        }
    }
}

pub(super) fn collect_evidence(state: &KernelState, selection: &mut BundleSelection) {
    for evidence in state.evidence.values() {
        if evidence
            .task_id
            .as_ref()
            .is_some_and(|task_id| selection.task_ids.contains(task_id))
            || evidence
                .artifact_id
                .as_ref()
                .is_some_and(|artifact_id| selection.artifact_ids.contains(artifact_id))
            || evidence
                .producer_agent_id
                .as_ref()
                .is_some_and(|agent_id| selection.agent_ids.contains(agent_id))
            || selection.evidence_ids.contains(&evidence.evidence_id)
        {
            selection.evidence_ids.insert(evidence.evidence_id.clone());
            if let Some(artifact_id) = &evidence.artifact_id {
                selection.artifact_ids.insert(artifact_id.clone());
            }
        }
    }
}

pub(super) fn collect_reviews(state: &KernelState, selection: &mut BundleSelection) {
    for review in state.reviews.values() {
        if selection.artifact_ids.contains(&review.artifact_id)
            || selection.agent_ids.contains(&review.reviewer_agent_id)
            || selection.review_ids.contains(&review.review_id)
        {
            selection.review_ids.insert(review.review_id.clone());
            selection
                .evidence_ids
                .extend(review.evidence_ids.iter().cloned());
        }
    }
    for finding in state.review_findings.values() {
        if selection.review_ids.contains(&finding.review_id)
            || finding
                .evidence_ids
                .iter()
                .any(|evidence_id| selection.evidence_ids.contains(evidence_id))
        {
            selection
                .review_finding_ids
                .insert(finding.finding_id.clone());
            selection
                .evidence_ids
                .extend(finding.evidence_ids.iter().cloned());
        }
    }
}

pub(super) fn collect_verifications(state: &KernelState, selection: &mut BundleSelection) {
    for verification in state.verifications.values() {
        if verification
            .artifact_id
            .as_ref()
            .is_some_and(|artifact_id| selection.artifact_ids.contains(artifact_id))
            || verification
                .final_artifact_id
                .as_ref()
                .is_some_and(|artifact_id| selection.artifact_ids.contains(artifact_id))
            || selection
                .agent_ids
                .contains(&verification.verifier_agent_id)
        {
            selection
                .verification_ids
                .insert(verification.verification_id.clone());
        }
    }
}

pub(super) fn collect_finals(state: &KernelState, selection: &mut BundleSelection) {
    for (task_id, final_submission) in &state.final_submissions {
        if selection.task_ids.contains(task_id) {
            selection
                .artifact_ids
                .extend(final_submission.changed_artifacts.iter().cloned());
            selection
                .approval_ids
                .extend(final_submission.approvals.iter().cloned());
            for entry in &final_submission.evidence_map {
                selection
                    .evidence_ids
                    .extend(entry.evidence_refs.iter().cloned());
            }
        }
    }
}

pub(super) fn collect_approvals(state: &KernelState, selection: &mut BundleSelection) {
    for approval in state.approvals.values() {
        if approval
            .task_id
            .as_ref()
            .is_some_and(|task_id| selection.task_ids.contains(task_id))
            || selection
                .agent_ids
                .contains(&approval.requested_by_agent_id)
            || selection.approval_ids.contains(&approval.approval_id)
        {
            selection.approval_ids.insert(approval.approval_id.clone());
        }
    }
}
