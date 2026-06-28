use agent_os_sys::*;

pub(super) fn core_scheduler_policies(now: &str) -> Vec<SchedulerPolicy> {
    [
        (
            "sched_foreground",
            "Foreground",
            QueueClass::Foreground,
            100,
        ),
        ("sched_background", "Background", QueueClass::Background, 10),
        ("sched_review", "Review", QueueClass::Review, 80),
    ]
    .into_iter()
    .map(|policy| SchedulerPolicy {
        scheduler_policy_id: policy.0.to_string(),
        status: ProfileStatus::Active,
        name: policy.1.to_string(),
        queue_class: policy.2,
        priority: policy.3,
        max_concurrent_children: 4,
        max_inflight_model_calls: Some(1),
        yield_policy: None,
        retry_policy: None,
        backoff_policy: None,
        starvation_window_ms: Some(30_000),
        budget_reservation_policy: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        superseded_by: None,
    })
    .collect()
}
