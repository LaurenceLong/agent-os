use crate::util::{parse_payload, to_value};
use crate::*;
use agent_os_sys::*;
use serde::Serialize;

impl Kernel {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn audit(
        &self,
        actor_type: AuditActorType,
        actor_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        reason: Option<String>,
        result: AuditResult,
    ) -> AgentOsResult<AuditEvent> {
        let audit = AuditEvent {
            audit_id: new_id("audit_"),
            event_id: String::new(),
            actor_type,
            actor_id: actor_id.to_string(),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            reason,
            result,
            created_at: now_rfc3339(),
        };
        self.emit(
            "AuditEventRecorded",
            "audit",
            &audit.audit_id,
            Some(actor_id.to_string()),
            None,
            None,
            None,
            &audit,
        )?;
        Ok(audit)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit<T: Serialize>(
        &self,
        event_type: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        agent_id: Option<String>,
        task_id: Option<String>,
        causation_id: Option<String>,
        correlation_id: Option<String>,
        payload: &T,
    ) -> AgentOsResult<EventEnvelope> {
        let event = EventEnvelope::new(
            event_type,
            aggregate_type,
            aggregate_id,
            agent_id,
            task_id,
            causation_id,
            correlation_id,
            to_value(payload)?,
        );
        self.store.append_projected(event.clone())?;
        self.apply_event(&event)?;
        Ok(event)
    }

    pub(crate) fn apply_event(&self, event: &EventEnvelope) -> AgentOsResult<()> {
        let mut state = self.write_state()?;
        match event.event_type.as_str() {
            "GoalRegistered" => {
                let goal: Goal = parse_payload(&event.payload)?;
                state.goals.insert(goal.goal_id.clone(), goal);
            }
            "TaskSpawned" => {
                let task: Task = parse_payload(&event.payload)?;
                if let Some(goal) = state.goals.get_mut(&task.goal_id) {
                    if goal.root_task_id.is_none() {
                        goal.root_task_id = Some(task.task_id.clone());
                        goal.updated_at = now_rfc3339();
                    }
                }
                state.tasks.insert(task.task_id.clone(), task);
            }
            "TaskUpdated" | "TaskCompleted" => {
                let task: Task = parse_payload(&event.payload)?;
                state.tasks.insert(task.task_id.clone(), task);
            }
            "BlackboardEntryRecorded" | "BlackboardPostSubmitted" | "BlackboardPostPublished" => {
                let entry: BlackboardEntry = parse_payload(&event.payload)?;
                state
                    .blackboard_entries
                    .insert(entry.entry_id.clone(), entry);
            }
            "BlackboardChannelCreated" => {
                let channel: BlackboardChannel = parse_payload(&event.payload)?;
                state
                    .blackboard_channels
                    .insert(channel.channel_id.clone(), channel);
            }
            "ContextLoaded" | "ContextInvalidated" => {
                let snapshot: ContextSnapshot = parse_payload(&event.payload)?;
                state
                    .context_snapshots
                    .insert(snapshot.context_id.clone(), snapshot);
            }
            "ContextCompacted" => {
                let compaction: ContextCompaction = parse_payload(&event.payload)?;
                state
                    .context_compactions
                    .insert(compaction.compaction_id.clone(), compaction);
            }
            "ThreadConfigured"
            | "ThreadStatusChanged"
            | "AgentStatePurged"
            | "ThreadGoalAccomplished"
            | "TurnStarted"
            | "CheckpointCommitted" => {
                let acb: AgentControlBlock = parse_payload(&event.payload)?;
                state.threads.insert(acb.thread_id.clone(), acb);
            }
            "AgentGoalAccomplished" => {
                let completion: AgentGoalCompletion = parse_payload(&event.payload)?;
                state
                    .threads
                    .insert(completion.thread.thread_id.clone(), completion.thread);
            }
            "AgentInvocationRecorded" => {
                let invocation: AgentInvocation = parse_payload(&event.payload)?;
                state
                    .agent_invocations
                    .insert(invocation.invocation_id.clone(), invocation);
            }
            "ThreadForked" => {
                let record: ThreadForkRecord = parse_payload(&event.payload)?;
                state.thread_forks.insert(record.fork_id.clone(), record);
            }
            "ThreadRolledBack" => {
                let record: ThreadRollbackRecord = parse_payload(&event.payload)?;
                state
                    .thread_rollbacks
                    .insert(record.rollback_id.clone(), record);
            }
            "AgentHookConfigured" | "AgentHookUpdated" => {
                let hook: AgentHook = parse_payload(&event.payload)?;
                state.agent_hooks.insert(hook.hook_id.clone(), hook);
            }
            "AgentControlCommandRecorded" => {
                let command: AgentControlCommand = parse_payload(&event.payload)?;
                state
                    .agent_control_commands
                    .insert(command.command_id.clone(), command);
            }
            "PermissionRequested" | "PermissionRequestResolved" => {
                let request: PermissionRequest = parse_payload(&event.payload)?;
                state
                    .permission_requests
                    .insert(request.permission_request_id.clone(), request);
            }
            "PermissionGranted" => {
                let grant: PermissionGrant = parse_payload(&event.payload)?;
                state
                    .permission_grants
                    .insert(grant.permission_grant_id.clone(), grant);
            }
            "CommunicationProfileAssigned" => {
                let profile: CommunicationProfile = parse_payload(&event.payload)?;
                state
                    .communication_profiles
                    .insert(profile.communication_profile_id.clone(), profile);
            }
            "ProviderProfileResolved" => {
                let decision: ProviderRouteDecision = parse_payload(&event.payload)?;
                state
                    .provider_route_decisions
                    .insert(event.aggregate_id.clone(), decision);
            }
            "ProviderStreamSessionOpened"
            | "ProviderStreamEventRecorded"
            | "ProviderUsageRecorded"
            | "ProviderStreamCompleted"
            | "ProviderStreamFailed"
            | "ProviderStreamCancelled" => {
                let session: ProviderStreamSession = parse_payload(&event.payload)?;
                state
                    .provider_stream_sessions
                    .insert(session.session_id.clone(), session);
            }
            "ProcessSessionStarted"
            | "ProcessSessionRunning"
            | "ProcessSessionExited"
            | "ProcessSessionFailed"
            | "ProcessSessionInterrupted"
            | "ProcessSessionTerminated"
            | "ProcessSessionTimedOut"
            | "ProcessSessionOrphaned" => {
                let session: ProcessSession = parse_payload(&event.payload)?;
                state
                    .process_sessions
                    .insert(session.process_id.clone(), session);
            }
            "ProcessOutputAppended" => {
                let chunk: ProcessOutputChunk = parse_payload(&event.payload)?;
                if let Some(session) = state.process_sessions.get_mut(&chunk.process_id) {
                    let stream = match chunk.stream {
                        ProcessOutputStreamName::Stdout => &mut session.stdout,
                        ProcessOutputStreamName::Stderr => &mut session.stderr,
                    };
                    stream.sequence = chunk.sequence;
                    stream.bytes = chunk.end_byte;
                    stream.cursor = chunk.end_byte;
                    session.updated_at = chunk.created_at.clone();
                }
                state.process_output_chunks.push(chunk);
            }
            "ProcessStdinWritten" => {
                let write: ProcessStdinWrite = parse_payload(&event.payload)?;
                if let Some(session) = state.process_sessions.get_mut(&write.process_id) {
                    session.updated_at = write.created_at.clone();
                }
                state.process_stdin_writes.push(write);
            }
            "CapabilityGranted" => {
                let cap: CapabilityToken = parse_payload(&event.payload)?;
                state.capabilities.insert(cap.capability_id.clone(), cap);
            }
            "ToolRegistered" => {
                let descriptor: ToolDescriptor = parse_payload(&event.payload)?;
                state
                    .tool_descriptors
                    .insert(descriptor.name.clone(), descriptor);
            }
            "ToolCallProposed" | "ToolCallStarted" | "ToolCallProgressed" | "ToolCallCompleted"
            | "ToolCallFailed" | "ToolCallDenied" | "ToolCallReconciled" => {
                let invocation: ToolInvocation = parse_payload(&event.payload)?;
                state
                    .tool_invocations
                    .insert(invocation.call_id.clone(), invocation);
            }
            "EnvironmentProvisioned" => {
                let env: ExecutionEnvironment = parse_payload(&event.payload)?;
                state.environments.insert(env.environment_id.clone(), env);
            }
            "EnvironmentLeaseGranted"
            | "EnvironmentLeaseReleased"
            | "EnvironmentLeaseReclaimed" => {
                let lease: EnvironmentLease = parse_payload(&event.payload)?;
                state
                    .environment_leases
                    .insert(lease.environment_lease_id.clone(), lease);
            }
            "ResourceLeaseGranted"
            | "ResourceLeaseDenied"
            | "ResourceLeaseReleased"
            | "ResourceLeaseReclaimed" => {
                let lease: ResourceLease = parse_payload(&event.payload)?;
                state
                    .resource_leases
                    .insert(lease.resource_lease_id.clone(), lease);
            }
            "ResourceSessionOpened" | "ResourceSessionClosed" => {
                let session: ResourceSession = parse_payload(&event.payload)?;
                state
                    .resource_sessions
                    .insert(session.session_id.clone(), session);
            }
            "AutomationScheduleCreated" | "AutomationScheduleUpdated" => {
                let schedule: AutomationSchedule = parse_payload(&event.payload)?;
                state
                    .automation_schedules
                    .insert(schedule.schedule_id.clone(), schedule);
            }
            "AutomationRunQueued"
            | "AutomationRunStarted"
            | "AutomationRunCompleted"
            | "AutomationRunFailed"
            | "AutomationRunCancelled" => {
                let run: AutomationRun = parse_payload(&event.payload)?;
                state.automation_runs.insert(run.run_id.clone(), run);
            }
            "BudgetLedgerCreated" | "BudgetDebited" | "BudgetExhausted" => {
                let ledger: BudgetLedger = parse_payload(&event.payload)?;
                state
                    .budget_ledgers
                    .insert(ledger.budget_ledger_id.clone(), ledger);
            }
            "CommunicationMessageDelivered" | "CommunicationMessageRejected" => {
                let msg: AgentMessage = parse_payload(&event.payload)?;
                state.messages.insert(msg.message_id.clone(), msg);
            }
            "MementoFragmentCreated"
            | "MementoFragmentArmed"
            | "MementoFragmentTriggered"
            | "MementoFragmentConsumed" => {
                let memento: MementoFragment = parse_payload(&event.payload)?;
                state.mementos.insert(memento.memento_id.clone(), memento);
            }
            "ArtifactCommitted" => {
                let artifact: Artifact = parse_payload(&event.payload)?;
                state
                    .artifacts
                    .insert(artifact.artifact_id.clone(), artifact);
            }
            "EvidenceAttached" => {
                let evidence: Evidence = parse_payload(&event.payload)?;
                state
                    .evidence
                    .insert(evidence.evidence_id.clone(), evidence);
            }
            "ReviewRequested" | "ReviewSubmitted" => {
                let review: Review = parse_payload(&event.payload)?;
                state.reviews.insert(review.review_id.clone(), review);
            }
            "ReviewFindingSubmitted" => {
                let finding: ReviewFinding = parse_payload(&event.payload)?;
                state
                    .review_findings
                    .insert(finding.finding_id.clone(), finding);
            }
            "VerificationSubmitted" => {
                let verification: Verification = parse_payload(&event.payload)?;
                state
                    .verifications
                    .insert(verification.verification_id.clone(), verification);
            }
            "ApprovalRequested" | "ApprovalRecorded" => {
                let approval: Approval = parse_payload(&event.payload)?;
                state
                    .approvals
                    .insert(approval.approval_id.clone(), approval);
            }
            "AuditEventRecorded" => {
                let audit: AuditEvent = parse_payload(&event.payload)?;
                state.audit_events.insert(audit.audit_id.clone(), audit);
            }
            "LockAcquired" | "LockReleased" | "LockExpired" | "LockForceReleased" => {
                let lock: Lock = parse_payload(&event.payload)?;
                state.locks.insert(lock.lock_id.clone(), lock);
            }
            "MemoryWriteProposed" | "MemoryWriteCommitted" | "MemoryInvalidated" => {
                let memory: MemoryRecord = parse_payload(&event.payload)?;
                state
                    .memory_records
                    .insert(memory.memory_id.clone(), memory);
            }
            "PackageInstalled" | "PackageEnabled" | "PackageDisabled" => {
                let install: PackageInstallRecord = parse_payload(&event.payload)?;
                state
                    .package_installs
                    .insert(install.manifest.package_name.clone(), install);
            }
            "PackageContributionRegistered" => {
                let contribution: PackageContributionRecord = parse_payload(&event.payload)?;
                state
                    .package_contributions
                    .insert(contribution.package_contribution_id.clone(), contribution);
            }
            "InstructionDocumentImported" => {
                let document: InstructionDocument = parse_payload(&event.payload)?;
                state
                    .instruction_documents
                    .insert(document.instruction_id.clone(), document);
            }
            "SkillDefinitionImported" => {
                let skill: SkillDefinition = parse_payload(&event.payload)?;
                state.skill_definitions.insert(skill.name.clone(), skill);
            }
            "CommandDefinitionImported" => {
                let command: CommandDefinition = parse_payload(&event.payload)?;
                state
                    .command_definitions
                    .insert(command.name.clone(), command);
            }
            "McpServerRegistered" => {
                let server: McpServerSpec = parse_payload(&event.payload)?;
                state.mcp_servers.insert(server.name.clone(), server);
            }
            "McpToolRegistered" => {
                let tool: McpToolDefinition = parse_payload(&event.payload)?;
                state.mcp_tools.insert(tool.model_tool_name.clone(), tool);
            }
            "ImportedAgentProfileRegistered" => {
                let profile: ImportedAgentProfile = parse_payload(&event.payload)?;
                state
                    .imported_agent_profiles
                    .insert(profile.name.clone(), profile);
            }
            "FinalSubmitted" => {
                let final_submission: FinalSubmission = parse_payload(&event.payload)?;
                state
                    .final_submissions
                    .insert(event.aggregate_id.clone(), final_submission);
            }
            "ThreadReconciled" => {
                let report: crate::recovery::ReconciliationReport = parse_payload(&event.payload)?;
                state
                    .reconciliation_reports
                    .insert(report.reconciliation_id.clone(), report);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn latest_events_for(&self, aggregate_id: &str) -> AgentOsResult<Vec<String>> {
        Ok(self
            .store
            .events_by_aggregate(aggregate_id)?
            .into_iter()
            .map(|event| event.event_id)
            .collect())
    }

    pub(crate) fn read_state(&self) -> AgentOsResult<std::sync::RwLockReadGuard<'_, KernelState>> {
        self.state
            .read()
            .map_err(|_| AgentOsError::Validation("kernel state read lock poisoned".to_string()))
    }

    pub(crate) fn write_state(
        &self,
    ) -> AgentOsResult<std::sync::RwLockWriteGuard<'_, KernelState>> {
        self.state
            .write()
            .map_err(|_| AgentOsError::Validation("kernel state write lock poisoned".to_string()))
    }
}
