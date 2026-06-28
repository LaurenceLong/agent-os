use crate::*;
use agent_os_sys::*;
use serde_json::{json, Value};

impl Kernel {
    pub fn resolve_provider_route(
        &self,
        request: StreamRequest,
    ) -> AgentOsResult<ProviderRouteDecision> {
        let decision = self.resolve_provider_route_decision(&request)?;
        let aggregate_id = new_id("route_decision_");
        self.emit(
            "ProviderProfileResolved",
            "provider_route",
            &aggregate_id,
            None,
            Some(request.task_id.clone()),
            None,
            None,
            &decision,
        )?;
        if decision.fallback_applied {
            self.emit(
                "ProviderFallbackApplied",
                "provider_route",
                &aggregate_id,
                None,
                Some(request.task_id.clone()),
                None,
                None,
                &decision,
            )?;
        }
        Ok(decision)
    }

    pub fn open_stream_session(
        &self,
        request: StreamRequest,
    ) -> AgentOsResult<ProviderStreamSession> {
        let decision = self.resolve_provider_route(request.clone())?;
        let session_id = new_id("stream_");
        let now = now_rfc3339();
        let mut session = ProviderStreamSession {
            session_id: session_id.clone(),
            request,
            route_decision: decision.clone(),
            status: ProviderStreamStatus::Open,
            stream_events: Vec::new(),
            usage: ProviderUsage::default(),
            created_at: now.clone(),
            completed_at: None,
        };
        if decision.fallback_applied {
            session.stream_events.push(provider_stream_event(
                &session_id,
                ProviderStreamEventType::ProviderFallback,
                json!({
                    "from": decision.fallback_from_model_alias,
                    "to": decision.selected_model_alias,
                    "provider_id": decision.provider_id
                }),
            ));
        }
        session.stream_events.push(provider_stream_event(
            &session_id,
            ProviderStreamEventType::StreamStarted,
            json!({
                "model_alias": decision.selected_model_alias,
                "provider_id": decision.provider_id,
                "provider_model_name": decision.provider_model_name
            }),
        ));
        self.emit(
            "ProviderStreamSessionOpened",
            "provider_stream_session",
            &session.session_id,
            None,
            Some(session.request.task_id.clone()),
            None,
            None,
            &session,
        )?;
        Ok(session)
    }

    pub fn record_provider_usage(
        &self,
        session_id: &str,
        usage: ProviderUsage,
    ) -> AgentOsResult<ProviderStreamSession> {
        let mut session = self.provider_stream_session(session_id)?;
        ensure_open_provider_session(&session)?;
        session.usage.input_tokens += usage.input_tokens;
        session.usage.output_tokens += usage.output_tokens;
        session.usage.cost += usage.cost;
        session.stream_events.push(provider_stream_event(
            session_id,
            ProviderStreamEventType::UsageUpdated,
            json!({
                "input_tokens": session.usage.input_tokens,
                "output_tokens": session.usage.output_tokens,
                "cost": session.usage.cost
            }),
        ));
        self.emit(
            "ProviderUsageRecorded",
            "provider_stream_session",
            &session.session_id,
            None,
            Some(session.request.task_id.clone()),
            None,
            None,
            &session,
        )?;
        Ok(session)
    }

    pub fn record_provider_stream_event(
        &self,
        session_id: &str,
        event_type: ProviderStreamEventType,
        payload: Value,
    ) -> AgentOsResult<ProviderStreamSession> {
        let mut session = self.provider_stream_session(session_id)?;
        ensure_open_provider_session(&session)?;
        session
            .stream_events
            .push(provider_stream_event(session_id, event_type, payload));
        self.emit(
            "ProviderStreamEventRecorded",
            "provider_stream_session",
            &session.session_id,
            None,
            Some(session.request.task_id.clone()),
            None,
            None,
            &session,
        )?;
        Ok(session)
    }

    pub fn complete_stream_session(
        &self,
        session_id: &str,
    ) -> AgentOsResult<ProviderStreamSession> {
        let mut session = self.provider_stream_session(session_id)?;
        ensure_open_provider_session(&session)?;
        session.status = ProviderStreamStatus::Completed;
        session.completed_at = Some(now_rfc3339());
        session.stream_events.push(provider_stream_event(
            session_id,
            ProviderStreamEventType::StreamCompleted,
            json!({
                "input_tokens": session.usage.input_tokens,
                "output_tokens": session.usage.output_tokens,
                "cost": session.usage.cost
            }),
        ));
        self.emit(
            "ProviderStreamCompleted",
            "provider_stream_session",
            &session.session_id,
            None,
            Some(session.request.task_id.clone()),
            None,
            None,
            &session,
        )?;
        Ok(session)
    }

    fn resolve_provider_route_decision(
        &self,
        request: &StreamRequest,
    ) -> AgentOsResult<ProviderRouteDecision> {
        let state = self.read_state()?;
        if !state.threads.contains_key(&request.thread_id) {
            return Err(AgentOsError::NotFound(format!(
                "thread {}",
                request.thread_id
            )));
        }
        if !state.tasks.contains_key(&request.task_id) {
            return Err(AgentOsError::NotFound(format!("task {}", request.task_id)));
        }
        let profile = state
            .provider_profiles
            .get(&request.provider_profile_id)
            .filter(|profile| profile.status == ProfileStatus::Active)
            .ok_or_else(|| {
                AgentOsError::NotFound(format!(
                    "active provider profile {}",
                    request.provider_profile_id
                ))
            })?;
        let routing = state
            .routing_policies
            .get(&request.model_routing_policy_id)
            .filter(|policy| policy.status == ProfileStatus::Active)
            .ok_or_else(|| {
                AgentOsError::NotFound(format!(
                    "active routing policy {}",
                    request.model_routing_policy_id
                ))
            })?;
        let preferred = request
            .requested_model_alias
            .clone()
            .or_else(|| role_routing_alias(routing, &request.role))
            .or_else(|| profile.default_model_alias.clone())
            .ok_or_else(|| AgentOsError::Validation("no model alias available".to_string()))?;
        ensure_profile_allows_alias(profile, &preferred)?;
        let (alias, fallback_from_model_alias) =
            match active_streaming_alias(&state, &preferred, request.output_schema.as_ref()) {
                Ok(alias) => (alias, None),
                Err(error) if !profile.fallback_chain.is_empty() => {
                    let fallback = profile
                        .fallback_chain
                        .iter()
                        .filter(|candidate| profile_allows_alias(profile, candidate))
                        .find_map(|candidate| {
                            active_streaming_alias(
                                &state,
                                candidate,
                                request.output_schema.as_ref(),
                            )
                            .ok()
                        })
                        .ok_or(error)?;
                    (fallback, Some(preferred.clone()))
                }
                Err(error) => return Err(error),
            };
        Ok(ProviderRouteDecision {
            provider_profile_id: profile.provider_profile_id.clone(),
            routing_policy_id: routing.routing_policy_id.clone(),
            requested_model_alias: request.requested_model_alias.clone(),
            selected_model_alias: alias.alias.clone(),
            provider_id: alias.provider_id.clone(),
            provider_model_name: alias.provider_model_name.clone(),
            fallback_applied: fallback_from_model_alias.is_some(),
            fallback_from_model_alias,
            resolved_at: now_rfc3339(),
        })
    }

    pub fn register_model_alias(
        &self,
        alias: &str,
        provider_id: &str,
        provider_model_name: &str,
        capabilities: Value,
        provider_profile_id: &str,
    ) -> AgentOsResult<()> {
        let now = now_rfc3339();
        let mut state = self.write_state()?;
        let model_alias = ModelAlias {
            model_alias_id: new_id("alias_"),
            alias: alias.to_string(),
            provider_id: provider_id.to_string(),
            provider_model_name: provider_model_name.to_string(),
            capabilities,
            limits: json!({
                "context_window": 128000,
                "max_output_tokens": 16000
            }),
            cost: json!({
                "input_per_1m": null,
                "output_per_1m": null
            }),
            status: "Active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        state.model_aliases.insert(alias.to_string(), model_alias);
        if let Some(profile) = state.provider_profiles.get_mut(provider_profile_id) {
            if !profile.allowed_model_aliases.iter().any(|a| a == alias) {
                profile.allowed_model_aliases.push(alias.to_string());
            }
        }
        Ok(())
    }

    fn provider_stream_session(&self, session_id: &str) -> AgentOsResult<ProviderStreamSession> {
        self.read_state()?
            .provider_stream_sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| AgentOsError::NotFound(format!("provider stream session {session_id}")))
    }
}

fn active_streaming_alias(
    state: &KernelState,
    alias: &str,
    output_schema: Option<&Value>,
) -> AgentOsResult<ModelAlias> {
    let alias = state
        .model_aliases
        .get(alias)
        .filter(|alias| alias.status == "Active")
        .cloned()
        .ok_or_else(|| {
            AgentOsError::PermissionDenied("model alias is not active or allowed".to_string())
        })?;
    if alias.capabilities.get("streaming").and_then(Value::as_bool) != Some(true) {
        return Err(AgentOsError::PermissionDenied(
            "model alias does not support streaming".to_string(),
        ));
    }
    if output_schema.is_some()
        && alias
            .capabilities
            .get("structured_output")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(AgentOsError::PermissionDenied(
            "model alias does not support structured output".to_string(),
        ));
    }
    Ok(alias)
}

fn ensure_profile_allows_alias(profile: &ProviderProfile, alias: &str) -> AgentOsResult<()> {
    if profile_allows_alias(profile, alias) {
        Ok(())
    } else {
        Err(AgentOsError::PermissionDenied(format!(
            "provider profile {} does not allow model alias {alias}",
            profile.provider_profile_id
        )))
    }
}

fn profile_allows_alias(profile: &ProviderProfile, alias: &str) -> bool {
    profile
        .allowed_model_aliases
        .iter()
        .any(|allowed| allowed == alias)
}

fn provider_stream_event(
    session_id: &str,
    event_type: ProviderStreamEventType,
    payload: Value,
) -> ProviderStreamEvent {
    ProviderStreamEvent {
        event_id: new_id("stream_evt_"),
        session_id: session_id.to_string(),
        event_type,
        payload,
        created_at: now_rfc3339(),
    }
}

fn ensure_open_provider_session(session: &ProviderStreamSession) -> AgentOsResult<()> {
    if session.status != ProviderStreamStatus::Open {
        return Err(AgentOsError::InvalidTransition(
            "provider stream session is not open".to_string(),
        ));
    }
    Ok(())
}

fn role_routing_alias(policy: &RoutingPolicy, role: &str) -> Option<String> {
    policy.rules.iter().find_map(|rule| {
        let matches_role = rule
            .get("when")
            .and_then(|when| when.get("role"))
            .and_then(Value::as_str)
            == Some(role);
        if matches_role {
            rule.get("use")
                .and_then(|use_rule| use_rule.get("model_alias"))
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            None
        }
    })
}
