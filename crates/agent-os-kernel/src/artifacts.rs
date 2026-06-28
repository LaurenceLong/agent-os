use crate::util::rfc3339_is_past;
use crate::*;
use agent_os_sys::*;
use serde_json::json;

impl Kernel {
    pub fn submit_final(
        &self,
        agent_id: &str,
        task_id: &str,
        final_submission: FinalSubmission,
    ) -> AgentOsResult<()> {
        self.submit_final_with_cause(agent_id, task_id, final_submission, None)
    }

    pub fn attach_evidence(&self, input: AttachEvidenceInput) -> AgentOsResult<Evidence> {
        self.attach_evidence_with_cause(input, None)
    }

    pub fn commit_artifact(&self, input: CommitArtifactInput) -> AgentOsResult<Artifact> {
        self.commit_artifact_with_cause(input, None)
    }

    pub(crate) fn attach_evidence_with_cause(
        &self,
        input: AttachEvidenceInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Evidence> {
        if !self.read_state()?.goals.contains_key(&input.goal_id) {
            return Err(AgentOsError::NotFound(format!("goal {}", input.goal_id)));
        }
        if let Some(task_id) = &input.task_id {
            if !self.read_state()?.tasks.contains_key(task_id) {
                return Err(AgentOsError::NotFound(format!("task {task_id}")));
            }
        }
        if let Some(artifact_id) = &input.artifact_id {
            if !self.read_state()?.artifacts.contains_key(artifact_id) {
                return Err(AgentOsError::NotFound(format!("artifact {artifact_id}")));
            }
        }
        let (blob_ref, content_hash, byte_len) = self.persist_evidence_blob(
            input.inline_bytes.as_deref(),
            input.blob_ref,
            input.content_hash,
        )?;
        let mut metadata = input.metadata;
        if let Some(byte_len) = byte_len {
            metadata["blob_byte_len"] = json!(byte_len);
        }
        let evidence = Evidence {
            evidence_id: new_id("evd_"),
            goal_id: input.goal_id,
            task_id: input.task_id,
            artifact_id: input.artifact_id,
            evidence_type: input.evidence_type,
            producer_agent_id: input.producer_agent_id,
            claim: input.claim,
            blob_ref,
            content_hash,
            metadata,
            status: EvidenceStatus::Active,
            created_at: now_rfc3339(),
            invalidated_by: None,
        };
        self.emit(
            "EvidenceAttached",
            "evidence",
            &evidence.evidence_id,
            evidence.producer_agent_id.clone(),
            evidence.task_id.clone(),
            causation_id,
            Some(evidence.goal_id.clone()),
            &evidence,
        )?;
        Ok(evidence)
    }

    pub(crate) fn commit_artifact_with_cause(
        &self,
        input: CommitArtifactInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<Artifact> {
        let state = self.read_state()?;
        if !state.tasks.contains_key(&input.task_id) {
            return Err(AgentOsError::NotFound(format!("task {}", input.task_id)));
        }
        if input.artifact_type == ArtifactType::Patch {
            let has_diff = input.evidence_ids.iter().any(|id| {
                state
                    .evidence
                    .get(id)
                    .is_some_and(|ev| ev.evidence_type == EvidenceType::DiffRef)
            });
            if !has_diff {
                return Err(AgentOsError::Validation(
                    "patch artifact requires diff evidence".to_string(),
                ));
            }
            ensure_workspace_mutation_allowed(&state, &input)?;
        }
        drop(state);
        let (blob_ref, content_hash, byte_len) = self.persist_artifact_blob(
            input.inline_bytes.as_deref(),
            input.blob_ref,
            input.content_hash,
        )?;
        let mut metadata = json!({
            "metadata": input.metadata,
            "evidence_ids": input.evidence_ids
        });
        if let Some(byte_len) = byte_len {
            metadata["blob_byte_len"] = json!(byte_len);
        }
        let now = now_rfc3339();
        let artifact = Artifact {
            artifact_id: new_id("art_"),
            goal_id: input.goal_id,
            task_id: input.task_id,
            owner_agent_id: input.owner_agent_id,
            artifact_type: input.artifact_type,
            version: 1,
            status: ArtifactStatus::Submitted,
            blob_ref,
            content_hash,
            metadata,
            created_at: now.clone(),
            updated_at: now,
            supersedes: input.supersedes,
        };
        self.emit(
            "ArtifactCommitted",
            "artifact",
            &artifact.artifact_id,
            Some(artifact.owner_agent_id.clone()),
            Some(artifact.task_id.clone()),
            causation_id,
            Some(artifact.goal_id.clone()),
            &artifact,
        )?;
        Ok(artifact)
    }

    pub(crate) fn submit_final_with_cause(
        &self,
        agent_id: &str,
        task_id: &str,
        final_submission: FinalSubmission,
        causation_id: Option<String>,
    ) -> AgentOsResult<()> {
        if final_submission.evidence_map.is_empty() {
            return Err(AgentOsError::Validation(
                "final answer without evidence map is rejected".to_string(),
            ));
        }
        let state = self.read_state()?;
        for entry in &final_submission.evidence_map {
            if entry.evidence_refs.is_empty() {
                return Err(AgentOsError::Validation(format!(
                    "claim '{}' lacks evidence refs",
                    entry.claim
                )));
            }
            for evidence_id in &entry.evidence_refs {
                let evidence = state
                    .evidence
                    .get(evidence_id)
                    .ok_or_else(|| AgentOsError::NotFound(format!("evidence {evidence_id}")))?;
                if evidence.status != EvidenceStatus::Active {
                    return Err(AgentOsError::Validation(format!(
                        "evidence {evidence_id} is not active"
                    )));
                }
            }
        }
        drop(state);
        self.emit(
            "FinalSubmitted",
            "task",
            task_id,
            Some(agent_id.to_string()),
            Some(task_id.to_string()),
            causation_id,
            None,
            &final_submission,
        )?;
        Ok(())
    }

    fn persist_artifact_blob(
        &self,
        inline_bytes: Option<&[u8]>,
        blob_ref: Option<String>,
        content_hash: Option<String>,
    ) -> AgentOsResult<(Option<String>, Option<String>, Option<usize>)> {
        match inline_bytes {
            Some(bytes) => {
                let store = self.artifact_blobs.as_ref().ok_or_else(|| {
                    AgentOsError::Validation(
                        "artifact inline bytes require an artifact blob store".to_string(),
                    )
                })?;
                let record = store.put_blob(bytes)?;
                Ok((
                    Some(record.blob_ref),
                    Some(record.content_hash),
                    Some(record.byte_len),
                ))
            }
            None => Ok((blob_ref, content_hash, None)),
        }
    }

    fn persist_evidence_blob(
        &self,
        inline_bytes: Option<&[u8]>,
        blob_ref: Option<String>,
        content_hash: Option<String>,
    ) -> AgentOsResult<(Option<String>, Option<String>, Option<usize>)> {
        match inline_bytes {
            Some(bytes) => {
                let store = self.evidence_blobs.as_ref().ok_or_else(|| {
                    AgentOsError::Validation(
                        "evidence inline bytes require an evidence blob store".to_string(),
                    )
                })?;
                let record = store.put_blob(bytes)?;
                Ok((
                    Some(record.blob_ref),
                    Some(record.content_hash),
                    Some(record.byte_len),
                ))
            }
            None => Ok((blob_ref, content_hash, None)),
        }
    }
}

fn ensure_workspace_mutation_allowed(
    state: &KernelState,
    input: &CommitArtifactInput,
) -> AgentOsResult<()> {
    let acb = state
        .threads
        .values()
        .find(|thread| thread.agent_id == input.owner_agent_id)
        .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", input.owner_agent_id)))?;
    if acb.task.task_id != input.task_id {
        return Err(AgentOsError::PermissionDenied(
            "artifact owner is not bound to task".to_string(),
        ));
    }
    let thread_sandbox = state
        .sandbox_profiles
        .get(&acb.config_snapshot.sandbox_profile_id)
        .ok_or_else(|| {
            AgentOsError::NotFound(format!(
                "sandbox profile {}",
                acb.config_snapshot.sandbox_profile_id
            ))
        })?;
    if !sandbox_allows_workspace_write(thread_sandbox) {
        return Err(AgentOsError::PermissionDenied(
            "artifact owner sandbox does not allow workspace mutation".to_string(),
        ));
    }
    for lease in state.environment_leases.values() {
        if lease.agent_id != input.owner_agent_id
            || lease.thread_id != acb.thread_id
            || lease.task_id != input.task_id
            || lease.status != EnvironmentLeaseStatus::Active
            || !matches!(
                lease.attach_mode,
                AttachMode::WorkspaceWrite | AttachMode::Exclusive
            )
        {
            continue;
        }
        if lease_is_expired(lease)? {
            continue;
        }
        if state
            .environments
            .get(&lease.environment_id)
            .and_then(|env| state.sandbox_profiles.get(&env.sandbox_profile_id))
            .is_some_and(sandbox_allows_workspace_write)
        {
            return Ok(());
        }
    }
    Err(AgentOsError::PermissionDenied(
        "workspace mutation requires an active writable environment lease".to_string(),
    ))
}

fn sandbox_allows_workspace_write(sandbox: &SandboxProfile) -> bool {
    sandbox.status == ProfileStatus::Active
        && matches!(
            sandbox.filesystem_mode,
            FilesystemMode::WorkspaceWrite | FilesystemMode::IsolatedWorktree
        )
}

fn lease_is_expired(lease: &EnvironmentLease) -> AgentOsResult<bool> {
    lease
        .expires_at
        .as_deref()
        .map(rfc3339_is_past)
        .transpose()
        .map(|expired| expired.unwrap_or(false))
}
