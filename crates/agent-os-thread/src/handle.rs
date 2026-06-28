use crate::{AgentOpAck, ThreadStatusSnapshot, TurnStartAck};
use agent_os_kernel::Kernel;
use agent_os_sys::*;

#[derive(Debug, Clone)]
pub struct AgentThreadHandle {
    kernel: Kernel,
    thread_id: String,
}

impl AgentThreadHandle {
    pub fn new(kernel: Kernel, thread_id: impl Into<String>) -> Self {
        Self {
            kernel,
            thread_id: thread_id.into(),
        }
    }

    pub fn submit_op(&self, op: AgentOp) -> AgentOsResult<AgentOpAck> {
        if op.thread_id != self.thread_id {
            return Ok(AgentOpAck {
                op_id: op.op_id,
                accepted: false,
                reason: Some("op targets a different thread".to_string()),
            });
        }
        let op_type = op.op_type.clone();
        match op_type.as_str() {
            "turn.start" => {
                let op_id = op.op_id.clone();
                self.try_start_turn(op).map(|_| AgentOpAck {
                    op_id,
                    accepted: true,
                    reason: None,
                })
            }
            "agent.suspend" => {
                self.kernel.transition_thread(
                    &self.thread_id,
                    ThreadStatus::Suspended,
                    Some("runtime op".to_string()),
                )?;
                Ok(AgentOpAck {
                    op_id: op.op_id,
                    accepted: true,
                    reason: None,
                })
            }
            "agent.resume" => {
                self.kernel.transition_thread(
                    &self.thread_id,
                    ThreadStatus::Ready,
                    Some("runtime op".to_string()),
                )?;
                Ok(AgentOpAck {
                    op_id: op.op_id,
                    accepted: true,
                    reason: None,
                })
            }
            "agent.checkpoint" => {
                self.checkpoint()?;
                Ok(AgentOpAck {
                    op_id: op.op_id,
                    accepted: true,
                    reason: None,
                })
            }
            other => Ok(AgentOpAck {
                op_id: op.op_id,
                accepted: false,
                reason: Some(format!("unknown runtime op {other}")),
            }),
        }
    }

    pub fn try_start_turn(&self, op: AgentOp) -> AgentOsResult<TurnStartAck> {
        if op.op_type != "turn.start" {
            return Err(AgentOsError::Validation(
                "try_start_turn requires a turn.start op".to_string(),
            ));
        }
        let acb = self.kernel.start_turn(&self.thread_id)?;
        Ok(TurnStartAck {
            thread_id: self.thread_id.clone(),
            turn_id: acb.active_turn.turn_id.ok_or_else(|| {
                AgentOsError::Validation("turn started without turn id".to_string())
            })?,
        })
    }

    pub fn steer_turn(&self, op: AgentOp) -> AgentOsResult<AgentOpAck> {
        let snapshot = self.status()?;
        if op.expected_turn_id != snapshot.active_turn.turn_id {
            return Ok(AgentOpAck {
                op_id: op.op_id,
                accepted: false,
                reason: Some("stale expected_turn_id".to_string()),
            });
        }
        Ok(AgentOpAck {
            op_id: op.op_id,
            accepted: true,
            reason: None,
        })
    }

    pub fn interrupt_turn(&self, expected_turn_id: &str) -> AgentOsResult<()> {
        let snapshot = self.status()?;
        if snapshot.active_turn.turn_id.as_deref() != Some(expected_turn_id) {
            return Err(AgentOsError::InvalidTransition(
                "cannot interrupt stale or missing turn".to_string(),
            ));
        }
        self.kernel.transition_thread(
            &self.thread_id,
            ThreadStatus::Interrupted,
            Some("runtime interrupt".to_string()),
        )?;
        Ok(())
    }

    pub fn inject_items(&self, items: Vec<AgentItem>) -> AgentOsResult<usize> {
        let snapshot = self.status()?;
        if snapshot.status != ThreadStatus::Running {
            return Err(AgentOsError::InvalidTransition(
                "items can be injected only while a turn is running".to_string(),
            ));
        }
        Ok(items.len())
    }

    pub fn status(&self) -> AgentOsResult<ThreadStatusSnapshot> {
        let state = self.kernel.state_snapshot()?;
        let acb = state
            .threads
            .get(&self.thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", self.thread_id)))?;
        Ok(ThreadStatusSnapshot {
            thread_id: self.thread_id.clone(),
            status: acb.status,
            active_turn: acb.active_turn.clone(),
        })
    }

    pub fn config_snapshot(&self) -> AgentOsResult<ThreadConfigSnapshot> {
        let state = self.kernel.state_snapshot()?;
        let acb = state
            .threads
            .get(&self.thread_id)
            .ok_or_else(|| AgentOsError::NotFound(format!("thread {}", self.thread_id)))?;
        Ok(acb.config_snapshot.clone())
    }

    pub fn subscribe_events(&self) -> AgentOsResult<Vec<EventEnvelope>> {
        Ok(self
            .kernel
            .events()?
            .into_iter()
            .filter(|event| event.aggregate_id == self.thread_id)
            .collect())
    }

    pub fn checkpoint(&self) -> AgentOsResult<String> {
        let checkpoint_id = new_id("ckpt_");
        self.kernel
            .record_checkpoint(&self.thread_id, checkpoint_id.clone())?;
        Ok(checkpoint_id)
    }

    pub fn shutdown(&self, reason: impl Into<String>) -> AgentOsResult<()> {
        self.kernel.transition_thread(
            &self.thread_id,
            ThreadStatus::Terminated,
            Some(reason.into()),
        )?;
        Ok(())
    }
}
