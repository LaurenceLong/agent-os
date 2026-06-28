use crate::util::hash_json;
use crate::*;
use agent_os_sys::*;

impl Kernel {
    pub fn visible_mementos_for_thread(
        &self,
        requester_thread_id: &str,
        owner_thread_id: &str,
    ) -> AgentOsResult<Vec<MementoFragment>> {
        if requester_thread_id != owner_thread_id {
            return Err(AgentOsError::PermissionDenied(
                "child or peer thread cannot read owner mementos".to_string(),
            ));
        }
        let mut mementos: Vec<_> = self
            .read_state()?
            .mementos
            .values()
            .filter(|m| m.owner_thread_id == owner_thread_id)
            .cloned()
            .collect();
        mementos.sort_by_key(|memento| std::cmp::Reverse(memento.projection.priority));
        Ok(mementos)
    }

    pub fn create_memento(&self, input: CreateMementoInput) -> AgentOsResult<MementoFragment> {
        self.create_memento_with_cause(input, None)
    }

    pub fn arm_memento(
        &self,
        owner_agent_id: &str,
        memento_id: &str,
    ) -> AgentOsResult<MementoFragment> {
        self.arm_memento_with_cause(owner_agent_id, memento_id, None)
    }

    pub fn consume_memento(
        &self,
        owner_agent_id: &str,
        memento_id: &str,
    ) -> AgentOsResult<MementoFragment> {
        self.consume_memento_with_cause(owner_agent_id, memento_id, None)
    }

    pub(crate) fn create_memento_with_cause(
        &self,
        input: CreateMementoInput,
        causation_id: Option<String>,
    ) -> AgentOsResult<MementoFragment> {
        let acb = self
            .read_state()?
            .threads
            .get(&input.owner_thread_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", input.owner_thread_id)))?;
        if acb.agent_id != input.owner_agent_id {
            return Err(AgentOsError::PermissionDenied(
                "only owner agent can create owner memento".to_string(),
            ));
        }
        let now = now_rfc3339();
        let memento = MementoFragment {
            memento_id: new_id("mmt_"),
            owner_agent_id: input.owner_agent_id,
            owner_thread_id: input.owner_thread_id,
            goal_id: input.goal_id,
            task_id: input.task_id,
            status: MementoStatus::Draft,
            anchor: input.anchor,
            content: input.content,
            projection: input.projection,
            immutability: MementoImmutability {
                content_hash: String::new(),
                committed_at: None,
                committed_by: None,
            },
            visibility: MementoVisibility {
                owner_only: true,
                child_visible: false,
            },
            links: input.links,
            supersession: MementoSupersession {
                supersedes: input.supersedes,
                superseded_by: None,
            },
            created_at: now.clone(),
            updated_at: now,
            expires_at: input.expires_at,
        };
        self.emit(
            "MementoFragmentCreated",
            "memento",
            &memento.memento_id,
            Some(memento.owner_agent_id.clone()),
            Some(memento.task_id.clone()),
            causation_id,
            Some(memento.goal_id.clone()),
            &memento,
        )?;
        Ok(memento)
    }

    pub(crate) fn arm_memento_with_cause(
        &self,
        owner_agent_id: &str,
        memento_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<MementoFragment> {
        let current = self
            .read_state()?
            .mementos
            .get(memento_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("memento {memento_id}")))?;
        if current.owner_agent_id != owner_agent_id {
            return Err(AgentOsError::PermissionDenied(
                "only owner can arm memento".to_string(),
            ));
        }
        if current.status != MementoStatus::Draft {
            return Err(AgentOsError::InvalidTransition(
                "only draft mementos can be armed".to_string(),
            ));
        }
        let mut next = current;
        next.status = MementoStatus::Armed;
        next.immutability.content_hash = hash_json(&next.content)?;
        next.immutability.committed_at = Some(now_rfc3339());
        next.immutability.committed_by = Some(owner_agent_id.to_string());
        next.updated_at = now_rfc3339();
        self.emit(
            "MementoFragmentArmed",
            "memento",
            &next.memento_id,
            Some(next.owner_agent_id.clone()),
            Some(next.task_id.clone()),
            causation_id,
            Some(next.goal_id.clone()),
            &next,
        )?;
        Ok(next)
    }

    pub(crate) fn consume_memento_with_cause(
        &self,
        owner_agent_id: &str,
        memento_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<MementoFragment> {
        let current = self
            .read_state()?
            .mementos
            .get(memento_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("memento {memento_id}")))?;
        if current.owner_agent_id != owner_agent_id {
            return Err(AgentOsError::PermissionDenied(
                "only owner can consume memento".to_string(),
            ));
        }
        if !matches!(
            current.status,
            MementoStatus::Triggered | MementoStatus::Projected | MementoStatus::Armed
        ) {
            return Err(AgentOsError::InvalidTransition(
                "memento cannot be consumed from current status".to_string(),
            ));
        }
        let mut next = current;
        next.status = MementoStatus::Consumed;
        next.updated_at = now_rfc3339();
        self.emit(
            "MementoFragmentConsumed",
            "memento",
            &next.memento_id,
            Some(next.owner_agent_id.clone()),
            Some(next.task_id.clone()),
            causation_id,
            Some(next.goal_id.clone()),
            &next,
        )?;
        Ok(next)
    }

    pub(crate) fn trigger_child_completion_mementos(
        &self,
        child_thread_id: &str,
        causation_id: Option<String>,
    ) -> AgentOsResult<()> {
        let candidates: Vec<_> = self
            .read_state()?
            .mementos
            .values()
            .filter(|m| {
                m.status == MementoStatus::Armed
                    && m.anchor.anchor_type == MementoAnchorType::ChildThreadCompleted
                    && m.anchor.anchor_ref.as_deref() == Some(child_thread_id)
            })
            .cloned()
            .collect();
        for mut memento in candidates {
            memento.status = MementoStatus::Triggered;
            memento.updated_at = now_rfc3339();
            self.emit(
                "MementoFragmentTriggered",
                "memento",
                &memento.memento_id,
                Some(memento.owner_agent_id.clone()),
                Some(memento.task_id.clone()),
                causation_id.clone(),
                Some(memento.goal_id.clone()),
                &memento,
            )?;
        }
        Ok(())
    }
}
