use crate::util::rfc3339_is_past;
use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn request_review(&self, input: RequestReviewInput) -> AgentOsResult<Review> {
        self.request_review_with_cause(input, None)
    }

    pub fn submit_review(&self, input: SubmitReviewInput) -> AgentOsResult<Review> {
        self.submit_review_with_cause(input, None)
    }

    pub fn submit_verification(
        &self,
        input: SubmitVerificationInput,
    ) -> AgentOsResult<Verification> {
        self.submit_verification_with_cause(input, None)
    }

    pub fn request_approval(&self, input: RequestApprovalInput) -> AgentOsResult<Approval> {
        self.request_approval_with_cause(input, None)
    }

    pub fn record_approval(&self, input: RecordApprovalInput) -> AgentOsResult<Approval> {
        self.record_approval_with_cause(input, None)
    }

    pub(crate) fn request_review_with_cause(
        &self,
        input: RequestReviewInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Review> {
        let artifact = self
            .read_state()?
            .artifacts
            .get(&input.artifact_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("artifact {}", input.artifact_id)))?;
        if artifact.owner_agent_id == input.reviewer_agent_id {
            return Err(AgentOsError::PermissionDenied(
                "ReviewerAgent must be independent from artifact owner".to_string(),
            ));
        }
        let review = Review {
            review_id: new_id("rev_"),
            artifact_id: artifact.artifact_id.clone(),
            artifact_version: artifact.version,
            reviewer_agent_id: input.reviewer_agent_id,
            status: ReviewStatus::Requested,
            focus: input.focus,
            verdict: ReviewVerdict::NeedsRevision,
            evidence_ids: Vec::new(),
            created_at: now_rfc3339(),
            submitted_at: None,
        };
        self.emit(
            "ReviewRequested",
            "review",
            &review.review_id,
            Some(review.reviewer_agent_id.clone()),
            Some(artifact.task_id),
            causation_id,
            Some(artifact.goal_id),
            &review,
        )?;
        Ok(review)
    }

    pub(crate) fn submit_review_with_cause(
        &self,
        input: SubmitReviewInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Review> {
        let current = self
            .read_state()?
            .reviews
            .get(&input.review_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("review {}", input.review_id)))?;
        if current.reviewer_agent_id != input.reviewer_agent_id {
            return Err(AgentOsError::PermissionDenied(
                "only assigned reviewer can submit review".to_string(),
            ));
        }
        let mut review = current;
        review.status = ReviewStatus::Submitted;
        review.verdict = input.verdict;
        review.evidence_ids = input.evidence_ids;
        review.submitted_at = Some(now_rfc3339());
        self.emit(
            "ReviewSubmitted",
            "review",
            &review.review_id,
            Some(review.reviewer_agent_id.clone()),
            None,
            causation_id.clone(),
            None,
            &review,
        )?;
        for finding in input.findings {
            let record = ReviewFinding {
                finding_id: new_id("finding_"),
                review_id: review.review_id.clone(),
                severity: finding.severity,
                title: finding.title,
                body: finding.body,
                location: finding.location,
                evidence_ids: finding.evidence_ids,
                status: FindingStatus::Open,
            };
            self.emit(
                "ReviewFindingSubmitted",
                "review_finding",
                &record.finding_id,
                Some(review.reviewer_agent_id.clone()),
                None,
                causation_id.clone(),
                None,
                &record,
            )?;
        }
        Ok(review)
    }

    pub(crate) fn submit_verification_with_cause(
        &self,
        input: SubmitVerificationInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Verification> {
        if let Some(artifact_id) = &input.artifact_id {
            let state = self.read_state()?;
            let artifact = state
                .artifacts
                .get(artifact_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("artifact {artifact_id}")))?;
            if artifact.owner_agent_id == input.verifier_agent_id {
                return Err(AgentOsError::PermissionDenied(
                    "WorkerAgent cannot verify its own artifact".to_string(),
                ));
            }
        }
        let verification = Verification {
            verification_id: new_id("ver_"),
            artifact_id: input.artifact_id,
            final_artifact_id: input.final_artifact_id,
            verifier_agent_id: input.verifier_agent_id,
            status: match input.verdict {
                VerificationVerdict::Pass => VerificationStatus::Passed,
                VerificationVerdict::Fail => VerificationStatus::Failed,
                VerificationVerdict::Inconclusive => VerificationStatus::Submitted,
            },
            checked_claims: input.checked_claims,
            unsupported_claims: input.unsupported_claims,
            stale_evidence_ids: input.stale_evidence_ids,
            verdict: input.verdict,
            created_at: now_rfc3339(),
            submitted_at: Some(now_rfc3339()),
        };
        self.emit(
            "VerificationSubmitted",
            "verification",
            &verification.verification_id,
            Some(verification.verifier_agent_id.clone()),
            None,
            causation_id,
            None,
            &verification,
        )?;
        Ok(verification)
    }

    pub(crate) fn request_approval_with_cause(
        &self,
        input: RequestApprovalInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Approval> {
        if input.scope.goal_id != input.goal_id {
            return Err(AgentOsError::Validation(
                "approval scope goal must match approval goal".to_string(),
            ));
        }
        if input.scope.task_id != input.task_id {
            return Err(AgentOsError::Validation(
                "approval scope task must match approval task".to_string(),
            ));
        }
        if let Some(expires_at) = &input.expires_at {
            if rfc3339_is_past(expires_at)? {
                return Err(AgentOsError::InvalidTransition(
                    "approval request is already expired".to_string(),
                ));
            }
        }
        let state = self.read_state()?;
        if !state.goals.contains_key(&input.goal_id) {
            return Err(AgentOsError::NotFound(format!("goal {}", input.goal_id)));
        }
        if let Some(task_id) = &input.task_id {
            let task = state
                .tasks
                .get(task_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("task {task_id}")))?;
            if task.goal_id != input.goal_id {
                return Err(AgentOsError::Validation(
                    "approval task does not belong to approval goal".to_string(),
                ));
            }
        }
        if !state
            .threads
            .values()
            .any(|thread| thread.agent_id == input.requested_by_agent_id)
        {
            return Err(AgentOsError::NotFound(format!(
                "agent {}",
                input.requested_by_agent_id
            )));
        }
        drop(state);
        let approval = Approval {
            approval_id: new_id("apr_"),
            goal_id: input.goal_id,
            task_id: input.task_id,
            requested_by_agent_id: input.requested_by_agent_id,
            approval_type: input.approval_type,
            scope: input.scope,
            risk_level: input.risk_level,
            status: ApprovalStatus::Requested,
            decision_by: None,
            decision_reason: None,
            created_at: now_rfc3339(),
            decided_at: None,
            expires_at: input.expires_at,
        };
        self.emit(
            "ApprovalRequested",
            "approval",
            &approval.approval_id,
            Some(approval.requested_by_agent_id.clone()),
            approval.task_id.clone(),
            causation_id,
            Some(approval.goal_id.clone()),
            &approval,
        )?;
        Ok(approval)
    }

    pub(crate) fn record_approval_with_cause(
        &self,
        input: RecordApprovalInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Approval> {
        let current = self
            .read_state()?
            .approvals
            .get(&input.approval_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("approval {}", input.approval_id)))?;
        if current.status != ApprovalStatus::Requested {
            return Err(AgentOsError::InvalidTransition(
                "approval decision can only be recorded once".to_string(),
            ));
        }
        if let Some(expires_at) = &current.expires_at {
            if rfc3339_is_past(expires_at)? {
                return Err(AgentOsError::InvalidTransition(
                    "approval request expired before decision".to_string(),
                ));
            }
        }
        let mut approval = current;
        approval.status = input.status;
        approval.decision_by = Some(input.decision_by);
        approval.decision_reason = input.decision_reason;
        approval.decided_at = Some(now_rfc3339());
        self.emit(
            "ApprovalRecorded",
            "approval",
            &approval.approval_id,
            Some(approval.requested_by_agent_id.clone()),
            approval.task_id.clone(),
            causation_id,
            Some(approval.goal_id.clone()),
            &approval,
        )?;
        Ok(approval)
    }
}
