use crate::error::sqlite_error;
use crate::events::insert_event_row;
use crate::store::SqliteStore;
use agent_os_store::{EventStore, ProjectionState, ProjectionStore};
use agent_os_sys::{
    AgentOsResult, ApprovalQueueProjection, ArtifactIndexProjection, AutomationRunProjection,
    AutomationScheduleProjection, ClientThread, EventEnvelope, EvidenceIndexProjection,
    ProjectionCheckpoint, ResourceSessionProjection, StatsQuery, StatsSnapshot, TimelineItem,
    TurnRecord,
};
use rusqlite::params;
use std::collections::BTreeMap;

impl ProjectionStore for SqliteStore {
    fn events_by_aggregate_type(&self, aggregate_type: &str) -> AgentOsResult<Vec<EventEnvelope>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT event_json FROM events WHERE aggregate_type = ?1 ORDER BY ordinal ASC")
            .map_err(sqlite_error)?;
        let rows = stmt
            .query_map(params![aggregate_type], |row| row.get::<_, String>(0))
            .map_err(sqlite_error)?;
        let mut events = Vec::new();
        for row in rows {
            let json = row.map_err(sqlite_error)?;
            events.push(serde_json::from_str(&json)?);
        }
        Ok(events)
    }

    fn clear_projections(&self) -> AgentOsResult<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sqlite_error)?;
        clear_projection_tables(&tx)?;
        tx.commit().map_err(sqlite_error)?;
        Ok(())
    }

    fn append_projected(&self, event: EventEnvelope) -> AgentOsResult<u64> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sqlite_error)?;
        let ordinal = insert_event_row(&tx, &event)?;
        let mut state = read_projection_state(&tx)?;
        state.apply_event(ordinal, &event)?;
        write_projection_state(&tx, &state)?;
        tx.commit().map_err(sqlite_error)?;
        Ok(ordinal)
    }

    fn project_event(&self, ordinal: u64, event: &EventEnvelope) -> AgentOsResult<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sqlite_error)?;
        let mut state = read_projection_state(&tx)?;
        state.apply_event(ordinal, event)?;
        write_projection_state(&tx, &state)?;
        tx.commit().map_err(sqlite_error)?;
        Ok(())
    }

    fn rebuild_projections(&self) -> AgentOsResult<()> {
        let events = self.all_events()?;
        let state = ProjectionState::rebuild(&events)?;
        self.replace_projection_state(&state)
    }

    fn thread_summaries(&self) -> AgentOsResult<Vec<ClientThread>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM thread_summaries ORDER BY updated_at ASC",
        )
    }

    fn turn_summaries(&self) -> AgentOsResult<Vec<TurnRecord>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM turn_summaries ORDER BY started_at ASC",
        )
    }

    fn timeline_items(&self, client_thread_id: Option<&str>) -> AgentOsResult<Vec<TimelineItem>> {
        let conn = self.lock()?;
        match client_thread_id {
            Some(thread_id) => read_projection_rows_with_param(
                &conn,
                "SELECT projection_json FROM timeline_items WHERE client_thread_id = ?1 ORDER BY created_at ASC",
                thread_id,
            ),
            None => read_projection_rows(
                &conn,
                "SELECT projection_json FROM timeline_items ORDER BY created_at ASC",
            ),
        }
    }

    fn stats_snapshot(&self, _query: StatsQuery) -> AgentOsResult<StatsSnapshot> {
        let conn = self.lock()?;
        let json = conn.query_row(
            "SELECT projection_json FROM stats_rollups WHERE scope_key = 'global'",
            [],
            |row| row.get::<_, String>(0),
        );
        match json {
            Ok(json) => Ok(serde_json::from_str(&json)?),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StatsSnapshot::default()),
            Err(error) => Err(sqlite_error(error)),
        }
    }

    fn approval_queue(&self) -> AgentOsResult<Vec<ApprovalQueueProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM approval_queue ORDER BY requested_at ASC",
        )
    }

    fn resource_sessions(&self) -> AgentOsResult<Vec<ResourceSessionProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM resource_sessions ORDER BY updated_at ASC",
        )
    }

    fn automation_schedules(&self) -> AgentOsResult<Vec<AutomationScheduleProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM automation_schedules ORDER BY updated_at ASC",
        )
    }

    fn automation_runs(&self) -> AgentOsResult<Vec<AutomationRunProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM automation_runs ORDER BY queued_at ASC",
        )
    }

    fn artifact_index(&self) -> AgentOsResult<Vec<ArtifactIndexProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM artifact_index ORDER BY created_at ASC",
        )
    }

    fn evidence_index(&self) -> AgentOsResult<Vec<EvidenceIndexProjection>> {
        let conn = self.lock()?;
        read_projection_rows(
            &conn,
            "SELECT projection_json FROM evidence_index ORDER BY created_at ASC",
        )
    }

    fn projection_checkpoint(
        &self,
        projection_name: &str,
    ) -> AgentOsResult<Option<ProjectionCheckpoint>> {
        let conn = self.lock()?;
        let row = conn.query_row(
            "
            SELECT projection_name, last_event_ordinal, updated_at
            FROM projection_checkpoints
            WHERE projection_name = ?1
            ",
            params![projection_name],
            |row| {
                let last_event_ordinal: i64 = row.get(1)?;
                Ok(ProjectionCheckpoint {
                    projection_name: row.get(0)?,
                    last_event_ordinal: last_event_ordinal as u64,
                    updated_at: row.get(2)?,
                })
            },
        );
        match row {
            Ok(checkpoint) => Ok(Some(checkpoint)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(sqlite_error(error)),
        }
    }
}

impl SqliteStore {
    fn replace_projection_state(&self, state: &ProjectionState) -> AgentOsResult<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sqlite_error)?;
        write_projection_state(&tx, state)?;
        tx.commit().map_err(sqlite_error)?;
        Ok(())
    }
}

fn write_projection_state(tx: &rusqlite::Connection, state: &ProjectionState) -> AgentOsResult<()> {
    clear_projection_tables(tx)?;
    for thread in state.threads.values() {
        tx.execute(
            "
                INSERT INTO thread_summaries(
                    client_thread_id, agent_thread_id, task_id, goal_id, title, status,
                    active_turn_id, archived, updated_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
            params![
                thread.client_thread_id,
                thread.agent_thread_id,
                thread.task_id,
                thread.goal_id,
                thread.title,
                format!("{:?}", thread.status),
                thread.active_turn_id,
                if thread.archived { 1 } else { 0 },
                thread.updated_at,
                serde_json::to_string(thread)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for turn in state.turns.values() {
        tx.execute(
            "
                INSERT INTO turn_summaries(
                    turn_id, client_thread_id, agent_thread_id, task_id, goal_id, status,
                    started_at, completed_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
            params![
                turn.turn_id,
                turn.client_thread_id,
                turn.agent_thread_id,
                turn.task_id,
                turn.goal_id,
                format!("{:?}", turn.status),
                turn.started_at,
                turn.completed_at,
                serde_json::to_string(turn)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for item in &state.timeline_items {
        tx.execute(
            "
                INSERT INTO timeline_items(
                    item_id, event_id, item_type, client_thread_id, agent_id, task_id,
                    turn_id, summary, created_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
            params![
                item.item_id,
                item.event_id,
                format!("{:?}", item.item_type),
                item.client_thread_id,
                item.agent_id,
                item.task_id,
                item.turn_id,
                item.summary,
                item.created_at,
                serde_json::to_string(item)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    tx.execute(
        "
            INSERT INTO stats_rollups(scope_key, updated_at, projection_json)
            VALUES('global', ?1, ?2)
            ",
        params![state.stats.updated_at, serde_json::to_string(&state.stats)?],
    )
    .map_err(sqlite_error)?;
    for approval in state.approvals.values() {
        tx.execute(
            "
                INSERT INTO approval_queue(
                    approval_id, client_thread_id, agent_id, task_id, status, requested_at,
                    resolved_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
            params![
                approval.approval_id,
                approval.client_thread_id,
                approval.agent_id,
                approval.task_id,
                approval.status,
                approval.requested_at,
                approval.resolved_at,
                serde_json::to_string(approval)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for resource in state.resources.values() {
        tx.execute(
            "
                INSERT INTO resource_sessions(
                    session_id, resource_type, client_thread_id, owner_agent_id, status,
                    lease_expires_at, updated_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
            params![
                resource.session_id,
                resource.resource_type,
                resource.client_thread_id,
                resource.owner_agent_id,
                resource.status,
                resource.lease_expires_at,
                resource.updated_at,
                serde_json::to_string(resource)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for schedule in state.automation_schedules.values() {
        tx.execute(
            "
                INSERT INTO automation_schedules(
                    schedule_id, name, kind, status, target_thread_id, workspace,
                    next_run_at, interval_seconds, updated_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
            params![
                schedule.schedule_id,
                schedule.name,
                format!("{:?}", schedule.kind),
                format!("{:?}", schedule.status),
                schedule.target_thread_id,
                schedule.workspace,
                schedule.next_run_at,
                schedule.interval_seconds.map(|value| value as i64),
                schedule.updated_at,
                serde_json::to_string(schedule)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for run in state.automation_runs.values() {
        tx.execute(
            "
                INSERT INTO automation_runs(
                    run_id, schedule_id, kind, status, target_thread_id, workspace,
                    scheduled_for, queued_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
            params![
                run.run_id,
                run.schedule_id,
                format!("{:?}", run.kind),
                format!("{:?}", run.status),
                run.target_thread_id,
                run.workspace,
                run.scheduled_for,
                run.queued_at,
                serde_json::to_string(run)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for artifact in state.artifacts.values() {
        tx.execute(
            "
                INSERT INTO artifact_index(
                    artifact_id, client_thread_id, agent_id, task_id, artifact_type,
                    created_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
            params![
                artifact.artifact_id,
                artifact.client_thread_id,
                artifact.agent_id,
                artifact.task_id,
                artifact.artifact_type,
                artifact.created_at,
                serde_json::to_string(artifact)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for evidence in state.evidence.values() {
        tx.execute(
            "
                INSERT INTO evidence_index(
                    evidence_id, client_thread_id, agent_id, task_id, evidence_type,
                    created_at, projection_json
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
            params![
                evidence.evidence_id,
                evidence.client_thread_id,
                evidence.agent_id,
                evidence.task_id,
                evidence.evidence_type,
                evidence.created_at,
                serde_json::to_string(evidence)?
            ],
        )
        .map_err(sqlite_error)?;
    }
    for checkpoint in state.checkpoints.values() {
        tx.execute(
            "
                INSERT INTO projection_checkpoints(
                    projection_name, last_event_ordinal, updated_at
                )
                VALUES(?1, ?2, ?3)
                ",
            params![
                checkpoint.projection_name,
                checkpoint.last_event_ordinal as i64,
                checkpoint.updated_at
            ],
        )
        .map_err(sqlite_error)?;
    }
    Ok(())
}

fn read_projection_state(conn: &rusqlite::Connection) -> AgentOsResult<ProjectionState> {
    let threads = read_projection_rows::<ClientThread>(
        conn,
        "SELECT projection_json FROM thread_summaries ORDER BY updated_at ASC",
    )?
    .into_iter()
    .map(|thread| (thread.client_thread_id.clone(), thread))
    .collect::<BTreeMap<_, _>>();
    let turns = read_projection_rows::<TurnRecord>(
        conn,
        "SELECT projection_json FROM turn_summaries ORDER BY started_at ASC",
    )?
    .into_iter()
    .map(|turn| (turn.turn_id.clone(), turn))
    .collect::<BTreeMap<_, _>>();
    let timeline_items = read_projection_rows::<TimelineItem>(
        conn,
        "SELECT projection_json FROM timeline_items ORDER BY created_at ASC",
    )?;
    let stats = read_stats_snapshot(conn)?;
    let approvals = read_projection_rows::<ApprovalQueueProjection>(
        conn,
        "SELECT projection_json FROM approval_queue ORDER BY requested_at ASC",
    )?
    .into_iter()
    .map(|approval| (approval.approval_id.clone(), approval))
    .collect::<BTreeMap<_, _>>();
    let resources = read_projection_rows::<ResourceSessionProjection>(
        conn,
        "SELECT projection_json FROM resource_sessions ORDER BY updated_at ASC",
    )?
    .into_iter()
    .map(|resource| (resource.session_id.clone(), resource))
    .collect::<BTreeMap<_, _>>();
    let automation_schedules = read_projection_rows::<AutomationScheduleProjection>(
        conn,
        "SELECT projection_json FROM automation_schedules ORDER BY updated_at ASC",
    )?
    .into_iter()
    .map(|schedule| (schedule.schedule_id.clone(), schedule))
    .collect::<BTreeMap<_, _>>();
    let automation_runs = read_projection_rows::<AutomationRunProjection>(
        conn,
        "SELECT projection_json FROM automation_runs ORDER BY queued_at ASC",
    )?
    .into_iter()
    .map(|run| (run.run_id.clone(), run))
    .collect::<BTreeMap<_, _>>();
    let artifacts = read_projection_rows::<ArtifactIndexProjection>(
        conn,
        "SELECT projection_json FROM artifact_index ORDER BY created_at ASC",
    )?
    .into_iter()
    .map(|artifact| (artifact.artifact_id.clone(), artifact))
    .collect::<BTreeMap<_, _>>();
    let evidence = read_projection_rows::<EvidenceIndexProjection>(
        conn,
        "SELECT projection_json FROM evidence_index ORDER BY created_at ASC",
    )?
    .into_iter()
    .map(|evidence| (evidence.evidence_id.clone(), evidence))
    .collect::<BTreeMap<_, _>>();
    let checkpoints = read_checkpoints(conn)?;
    Ok(ProjectionState {
        threads,
        turns,
        timeline_items,
        stats,
        approvals,
        resources,
        automation_schedules,
        automation_runs,
        artifacts,
        evidence,
        checkpoints,
    })
}

fn read_stats_snapshot(conn: &rusqlite::Connection) -> AgentOsResult<StatsSnapshot> {
    let json = conn.query_row(
        "SELECT projection_json FROM stats_rollups WHERE scope_key = 'global'",
        [],
        |row| row.get::<_, String>(0),
    );
    match json {
        Ok(json) => Ok(serde_json::from_str(&json)?),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StatsSnapshot::default()),
        Err(error) => Err(sqlite_error(error)),
    }
}

fn read_checkpoints(
    conn: &rusqlite::Connection,
) -> AgentOsResult<BTreeMap<String, ProjectionCheckpoint>> {
    let mut stmt = conn
        .prepare(
            "
            SELECT projection_name, last_event_ordinal, updated_at
            FROM projection_checkpoints
            ORDER BY projection_name ASC
            ",
        )
        .map_err(sqlite_error)?;
    let rows = stmt
        .query_map([], |row| {
            let last_event_ordinal: i64 = row.get(1)?;
            Ok(ProjectionCheckpoint {
                projection_name: row.get(0)?,
                last_event_ordinal: last_event_ordinal as u64,
                updated_at: row.get(2)?,
            })
        })
        .map_err(sqlite_error)?;
    let mut checkpoints = BTreeMap::new();
    for row in rows {
        let checkpoint = row.map_err(sqlite_error)?;
        checkpoints.insert(checkpoint.projection_name.clone(), checkpoint);
    }
    Ok(checkpoints)
}

fn clear_projection_tables(conn: &rusqlite::Connection) -> AgentOsResult<()> {
    conn.execute_batch(
        "
        DELETE FROM thread_summaries;
        DELETE FROM turn_summaries;
        DELETE FROM timeline_items;
        DELETE FROM stats_rollups;
        DELETE FROM approval_queue;
        DELETE FROM resource_sessions;
        DELETE FROM automation_schedules;
        DELETE FROM automation_runs;
        DELETE FROM artifact_index;
        DELETE FROM evidence_index;
        DELETE FROM projection_checkpoints;
        ",
    )
    .map_err(sqlite_error)
}

fn read_projection_rows<T>(conn: &rusqlite::Connection, sql: &str) -> AgentOsResult<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut stmt = conn.prepare(sql).map_err(sqlite_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sqlite_error)?;
    read_projection_json(rows)
}

fn read_projection_rows_with_param<T>(
    conn: &rusqlite::Connection,
    sql: &str,
    value: &str,
) -> AgentOsResult<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut stmt = conn.prepare(sql).map_err(sqlite_error)?;
    let rows = stmt
        .query_map(params![value], |row| row.get::<_, String>(0))
        .map_err(sqlite_error)?;
    read_projection_json(rows)
}

fn read_projection_json<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
) -> AgentOsResult<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut records = Vec::new();
    for row in rows {
        let json = row.map_err(sqlite_error)?;
        records.push(serde_json::from_str(&json)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_store::{EventStore, ProjectionStore};
    use agent_os_sys::{StatsQuery, TimelineItemType, ToolCallStatus, ToolInvocation};
    use serde_json::json;

    #[test]
    fn rebuild_materializes_timeline_stats_and_checkpoint() {
        let store = SqliteStore::in_memory().unwrap();
        let invocation = ToolInvocation {
            call_id: "call_1".to_string(),
            tool_id: "tool_1".to_string(),
            tool_name: "workspace_write".to_string(),
            agent_id: "agt_1".to_string(),
            task_id: "task_1".to_string(),
            status: ToolCallStatus::Completed,
            risk_level: 2,
            input: json!({"path": "README.md"}),
            output: Some(json!({"ok": true})),
            evidence_ids: vec!["evd_1".to_string()],
            audit_refs: Vec::new(),
            created_at: "2026-06-30T00:00:00Z".to_string(),
            completed_at: Some("2026-06-30T00:00:01Z".to_string()),
        };
        let event = EventEnvelope::new(
            "ToolCallCompleted",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            None,
            Some("goal_1".to_string()),
            serde_json::to_value(&invocation).unwrap(),
        );

        store.append(event).unwrap();
        store.rebuild_projections().unwrap();

        let items = store.timeline_items(None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_type, TimelineItemType::ToolUpdated);
        assert_eq!(items[0].agent_id.as_deref(), Some("agt_1"));

        let stats = store.stats_snapshot(StatsQuery::default()).unwrap();
        assert_eq!(stats.tool_calls, 1);
        assert_eq!(stats.tool_successes, 1);
        assert_eq!(stats.tool_failures, 0);

        let checkpoint = store
            .projection_checkpoint("stats_rollups")
            .unwrap()
            .expect("stats checkpoint");
        assert_eq!(checkpoint.last_event_ordinal, 1);
    }

    #[test]
    fn append_projected_persists_event_projection_and_checkpoint_together() {
        let store = SqliteStore::in_memory().unwrap();
        let invocation = ToolInvocation {
            call_id: "call_projected".to_string(),
            tool_id: "tool_apply_patch".to_string(),
            tool_name: "apply_patch".to_string(),
            agent_id: "agt_1".to_string(),
            task_id: "task_1".to_string(),
            status: ToolCallStatus::Completed,
            risk_level: 2,
            input: json!({"patch": "*** Begin Patch\n*** End Patch\n"}),
            output: Some(json!({"ok": true})),
            evidence_ids: Vec::new(),
            audit_refs: Vec::new(),
            created_at: "2026-06-30T00:00:00Z".to_string(),
            completed_at: Some("2026-06-30T00:00:01Z".to_string()),
        };
        let event = EventEnvelope::new(
            "ToolCallCompleted",
            "tool_invocation",
            &invocation.call_id,
            Some(invocation.agent_id.clone()),
            Some(invocation.task_id.clone()),
            None,
            Some("goal_1".to_string()),
            serde_json::to_value(&invocation).unwrap(),
        );

        let ordinal = store.append_projected(event.clone()).unwrap();

        assert_eq!(ordinal, 1);
        assert_eq!(store.all_events().unwrap()[0].event_id, event.event_id);
        assert_eq!(
            store
                .stats_snapshot(StatsQuery::default())
                .unwrap()
                .tool_successes,
            1
        );
        assert_eq!(
            store
                .projection_checkpoint("event_stream")
                .unwrap()
                .unwrap()
                .last_event_ordinal,
            1
        );
    }
}
