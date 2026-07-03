use crate::common::*;

#[test]
fn selected_task_bundle_exports_replayable_projection_slice() {
    let fx = fixture();
    attach_writable_environment(&fx);
    let diff = fx
        .kernel
        .attach_evidence(evidence_input(&fx, EvidenceType::DiffRef))
        .unwrap();
    let artifact = fx
        .kernel
        .commit_artifact(CommitArtifactInput {
            goal_id: fx.goal.goal_id.clone(),
            task_id: fx.task.task_id.clone(),
            owner_agent_id: fx.worker.agent_id.clone(),
            artifact_type: ArtifactType::Patch,
            blob_ref: Some("patch://demo".to_string()),
            content_hash: Some("patch-hash".to_string()),
            inline_bytes: None,
            metadata: json!({"path": "src/lib.rs"}),
            evidence_ids: vec![diff.evidence_id.clone()],
            supersedes: None,
        })
        .unwrap();
    fx.kernel
        .complete_task(CompleteTaskInput {
            task_id: fx.task.task_id.clone(),
            artifact_ids: vec![artifact.artifact_id.clone()],
            evidence_ids: vec![diff.evidence_id.clone()],
        })
        .unwrap();
    fx.kernel
        .submit_final(
            &fx.worker.agent_id,
            &fx.task.task_id,
            FinalSubmission {
                summary: "bundle export completed".to_string(),
                changed_artifacts: vec![artifact.artifact_id.clone()],
                evidence_map: vec![EvidenceMapEntry {
                    claim: "patch has diff evidence".to_string(),
                    evidence_refs: vec![diff.evidence_id.clone()],
                }],
                unverified_claims: Vec::new(),
                known_risks: Vec::new(),
                tests_run: Vec::new(),
                tests_not_run: vec!["not required for export conformance".to_string()],
                approvals: Vec::new(),
            },
        )
        .unwrap();

    let bundle = fx.kernel.export_task_bundle(&fx.task.task_id).unwrap();
    assert_eq!(bundle.bundle_kind, BundleKind::Task);
    assert_eq!(bundle.abi_version, ABI_VERSION);
    assert_eq!(bundle.root_task_id, fx.task.task_id);
    assert_eq!(bundle.goal_id, fx.goal.goal_id);
    assert_eq!(bundle.projection_snapshot.tasks.len(), 1);
    assert_eq!(bundle.projection_snapshot.agent_invocations.len(), 1);
    assert_eq!(bundle.projection_snapshot.artifacts.len(), 1);
    assert_eq!(bundle.projection_snapshot.evidence.len(), 1);
    assert_eq!(bundle.projection_snapshot.final_submissions.len(), 1);
    assert!(bundle
        .profile_snapshot
        .role_profiles
        .iter()
        .any(|profile| profile.role_profile_id == "role_producer"));
    assert!(bundle
        .profile_snapshot
        .permission_profiles
        .iter()
        .any(|profile| profile.permission_profile_id == "perm_producer"));
    for required_event in [
        "GoalRegistered",
        "TaskSpawned",
        "AgentInvocationRecorded",
        "ThreadConfigured",
        "EvidenceAttached",
        "ArtifactCommitted",
        "TaskCompleted",
        "FinalSubmitted",
    ] {
        assert!(
            bundle
                .events
                .iter()
                .any(|event| event.event_type == required_event),
            "missing {required_event}"
        );
    }

    let replayed = Kernel::from_events(&bundle.events).unwrap();
    let replayed_state = replayed.state_snapshot().unwrap();
    assert!(replayed_state.tasks.contains_key(&fx.task.task_id));
    assert!(replayed_state
        .agent_invocations
        .contains_key(&fx.worker.invocation_id));
    assert!(replayed_state.artifacts.contains_key(&artifact.artifact_id));
    assert!(replayed_state.evidence.contains_key(&diff.evidence_id));
    assert!(replayed_state
        .final_submissions
        .contains_key(&fx.task.task_id));
    assert_eq!(bundle.replay_summary.event_count, bundle.events.len());
}

#[test]
fn replay_bundle_marks_export_kind_without_losing_events() {
    let fx = fixture();
    let task_bundle = fx.kernel.export_task_bundle(&fx.task.task_id).unwrap();
    let replay_bundle = fx.kernel.export_replay_bundle(&fx.task.task_id).unwrap();
    assert_eq!(replay_bundle.bundle_kind, BundleKind::Replay);
    assert_eq!(replay_bundle.root_task_id, task_bundle.root_task_id);
    assert_eq!(replay_bundle.task_ids, task_bundle.task_ids);
    assert_eq!(replay_bundle.events.len(), task_bundle.events.len());
}
