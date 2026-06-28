use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn load_context(&self, input: LoadContextInput) -> AgentOsResult<ContextSnapshot> {
        self.load_context_with_cause(input, None)
    }

    pub(crate) fn load_context_with_cause(
        &self,
        input: LoadContextInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<ContextSnapshot> {
        self.validate_context_load(&input)?;
        let snapshot = ContextSnapshot {
            context_id: new_id("ctx_"),
            agent_id: input.agent_id,
            task_id: input.task_id,
            loaded_refs: input.loaded_refs,
            summary_artifact_id: input.summary_artifact_id,
            freshness: input.freshness,
            pollution_score: input.pollution_score,
            token_estimate: input.token_estimate,
            created_at: now_rfc3339(),
            invalidated_at: None,
        };
        self.emit(
            "ContextLoaded",
            "context",
            &snapshot.context_id,
            Some(snapshot.agent_id.clone()),
            Some(snapshot.task_id.clone()),
            causation_id,
            None,
            &snapshot,
        )?;
        Ok(snapshot)
    }

    fn validate_context_load(&self, input: &LoadContextInput) -> AgentOsResult<()> {
        if input.loaded_refs.is_empty() && input.summary_artifact_id.is_none() {
            return Err(AgentOsError::Validation(
                "context load requires at least one loaded ref or summary artifact".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&input.pollution_score) {
            return Err(AgentOsError::Validation(
                "context pollution score must be between 0.0 and 1.0".to_string(),
            ));
        }
        let state = self.read_state()?;
        let acb = state
            .threads
            .values()
            .find(|thread| thread.agent_id == input.agent_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("agent {}", input.agent_id)))?;
        if acb.task.task_id != input.task_id {
            return Err(AgentOsError::PermissionDenied(
                "context load task must match agent thread task".to_string(),
            ));
        }
        if let Some(artifact_id) = &input.summary_artifact_id {
            let artifact = state
                .artifacts
                .get(artifact_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("artifact {artifact_id}")))?;
            if artifact.task_id != input.task_id {
                return Err(AgentOsError::Validation(
                    "context summary artifact must belong to task".to_string(),
                ));
            }
        }
        Ok(())
    }

    // ---- Memory write policy ----
    //
    // Memory writes require provenance and pass through a Proposed -> Active
    // gate so proposed memory is never authoritative
    // (`docs/10-kernel-design/kernel-data-model.md:849-850`).

    pub fn propose_memory_write(
        &self,
        input: ProposeMemoryWriteInput,
    ) -> AgentOsResult<MemoryRecord> {
        self.propose_memory_write_with_cause(input, None)
    }

    pub(crate) fn propose_memory_write_with_cause(
        &self,
        input: ProposeMemoryWriteInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<MemoryRecord> {
        if input.namespace.is_empty() {
            return Err(AgentOsError::Validation(
                "memory write requires a namespace".to_string(),
            ));
        }
        // Provenance: a proposed memory must carry at least one source
        // evidence id so every long-term memory write has a source
        // (`docs/00-foundation/agent-collaboration-theory.md:510`).
        if input.source_evidence_ids.is_empty() {
            return Err(AgentOsError::Validation(
                "memory write requires source evidence provenance".to_string(),
            ));
        }
        let state = self.read_state()?;
        for evidence_id in &input.source_evidence_ids {
            let evidence = state
                .evidence
                .get(evidence_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("evidence {evidence_id}")))?;
            if evidence.status != EvidenceStatus::Active {
                return Err(AgentOsError::Validation(format!(
                    "memory provenance evidence {evidence_id} is not active"
                )));
            }
        }
        drop(state);
        let record = MemoryRecord {
            memory_id: new_id("mem_"),
            namespace: input.namespace,
            status: MemoryStatus::Proposed,
            content: input.content,
            source_evidence_ids: input.source_evidence_ids,
            created_by_agent_id: Some(input.created_by_agent_id),
            approved_by: None,
            created_at: now_rfc3339(),
            activated_at: None,
            superseded_by: None,
        };
        self.emit(
            "MemoryWriteProposed",
            "memory",
            &record.memory_id,
            record.created_by_agent_id.clone(),
            None,
            causation_id,
            None,
            &record,
        )?;
        Ok(record)
    }

    pub fn commit_memory_write(
        &self,
        input: CommitMemoryWriteInput,
    ) -> AgentOsResult<MemoryRecord> {
        self.commit_memory_write_with_cause(input, None)
    }

    pub(crate) fn commit_memory_write_with_cause(
        &self,
        input: CommitMemoryWriteInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<MemoryRecord> {
        let record = self
            .read_state()?
            .memory_records
            .get(&input.memory_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("memory {}", input.memory_id)))?;
        if record.status != MemoryStatus::Proposed {
            return Err(AgentOsError::InvalidTransition(format!(
                "memory {} is {:?}, not Proposed",
                input.memory_id, record.status
            )));
        }
        let mut activated = record;
        activated.status = MemoryStatus::Active;
        activated.approved_by = Some(input.approved_by);
        activated.activated_at = Some(now_rfc3339());
        self.emit(
            "MemoryWriteCommitted",
            "memory",
            &activated.memory_id,
            activated.created_by_agent_id.clone(),
            None,
            causation_id,
            None,
            &activated,
        )?;
        Ok(activated)
    }

    pub fn invalidate_memory(&self, memory_id: &str) -> AgentOsResult<MemoryRecord> {
        self.invalidate_memory_with_cause(memory_id, None)
    }

    pub(crate) fn invalidate_memory_with_cause(
        &self,
        memory_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<MemoryRecord> {
        let record = self
            .read_state()?
            .memory_records
            .get(memory_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("memory {memory_id}")))?;
        let mut invalidated = record;
        invalidated.status = MemoryStatus::Invalidated;
        self.emit(
            "MemoryInvalidated",
            "memory",
            &invalidated.memory_id,
            invalidated.created_by_agent_id.clone(),
            None,
            causation_id,
            None,
            &invalidated,
        )?;
        Ok(invalidated)
    }

    // ---- Context summary and invalidation ----

    /// Mark a previously loaded context snapshot stale. The snapshot must
    /// exist; stale context is marked, not silently reused
    /// (`docs/10-kernel-design/agent-thread-core-module.md:541`).
    pub fn invalidate_context(&self, context_id: &str) -> AgentOsResult<ContextSnapshot> {
        self.invalidate_context_with_cause(context_id, None)
    }

    pub(crate) fn invalidate_context_with_cause(
        &self,
        context_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<ContextSnapshot> {
        let snapshot = self
            .read_state()?
            .context_snapshots
            .get(context_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("context {context_id}")))?;
        let mut invalidated = snapshot;
        invalidated.freshness = ContextFreshness::Stale;
        invalidated.invalidated_at = Some(now_rfc3339());
        self.emit(
            "ContextInvalidated",
            "context",
            &invalidated.context_id,
            Some(invalidated.agent_id.clone()),
            Some(invalidated.task_id.clone()),
            causation_id,
            None,
            &invalidated,
        )?;
        Ok(invalidated)
    }

    // ---- Compaction with replacement provenance ----

    /// Compact a thread's context window. Emits a `ContextCompacted` event
    /// that links the summary to the superseded context refs, satisfying the
    /// replacement-provenance requirement
    /// (`docs/10-kernel-design/agent-thread-core-module.md:542-543, 833`).
    pub fn compact_context(&self, input: CompactContextInput) -> AgentOsResult<ContextCompaction> {
        self.compact_context_with_cause(input, None)
    }

    pub(crate) fn compact_context_with_cause(
        &self,
        input: CompactContextInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<ContextCompaction> {
        let state = self.read_state()?;
        let acb = state
            .threads
            .get(&input.thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", input.thread_id)))?;
        if acb.task.task_id != input.task_id {
            return Err(AgentOsError::PermissionDenied(
                "context compaction task must match thread task".to_string(),
            ));
        }
        if let Some(artifact_id) = &input.summary_artifact_id {
            let artifact = state
                .artifacts
                .get(artifact_id)
                .ok_or_else(|| AgentOsError::NotFound(format!("artifact {artifact_id}")))?;
            if artifact.task_id != input.task_id {
                return Err(AgentOsError::Validation(
                    "compaction summary artifact must belong to task".to_string(),
                ));
            }
        }
        drop(state);
        let compaction = ContextCompaction {
            compaction_id: new_id("cmpct_"),
            thread_id: input.thread_id,
            agent_id: input.agent_id,
            task_id: input.task_id,
            summary_artifact_id: input.summary_artifact_id,
            superseded_refs: input.superseded_refs,
            token_estimate: input.token_estimate,
            created_at: now_rfc3339(),
        };
        self.emit(
            "ContextCompacted",
            "context",
            &compaction.compaction_id,
            Some(compaction.agent_id.clone()),
            Some(compaction.task_id.clone()),
            causation_id,
            None,
            &compaction,
        )?;
        Ok(compaction)
    }
}
