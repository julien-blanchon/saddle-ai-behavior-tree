use saddle_ai_behavior_tree::BehaviorTreeAgent;
use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["smoke_launch", "stress_test_metrics"]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "stress_test_metrics" => Some(stress_test_metrics()),
        _ => None,
    }
}

fn pane_total_ticks(pane: &crate::StressPane) -> Option<u64> {
    pane.total_ticks.parse().ok()
}

fn pane_agent_count(pane: &crate::StressPane) -> Option<usize> {
    pane.agents.parse().ok()
}

fn smoke_launch() -> Scenario {
    Scenario::builder("smoke_launch")
        .description("Boot the stress test, verify all agents spawn, and capture the initial metrics overlay.")
        .then(Action::WaitFrames(20))
        .then(assertions::entity_count::<BehaviorTreeAgent>(
            "2048 stress agents spawned",
            2048,
        ))
        .then(assertions::resource_satisfies::<crate::StressPane>(
            "pane reports 2048 agents",
            |pane| pane_agent_count(pane) == Some(2048),
        ))
        .then(Action::Screenshot("stress_boot".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("smoke_launch"))
        .build()
}

fn stress_test_metrics() -> Scenario {
    Scenario::builder("stress_test_metrics")
        .description("Let the stress scene run long enough for the tick counters to accumulate, then verify the overlay reports sustained work at the full 2048-agent scale.")
        .then(Action::WaitFrames(90))
        .then(assertions::entity_count::<BehaviorTreeAgent>(
            "2048 stress agents remain alive",
            2048,
        ))
        .then(assertions::resource_satisfies::<crate::StressPane>(
            "pane still reports 2048 agents",
            |pane| pane_agent_count(pane) == Some(2048),
        ))
        .then(assertions::resource_satisfies::<crate::StressPane>(
            "total tick counter grows well beyond startup noise",
            |pane| pane_total_ticks(pane).is_some_and(|ticks| ticks > 10_000),
        ))
        .then(assertions::resource_satisfies::<crate::StressPane>(
            "frame time monitor stays parseable and finite",
            |pane| pane.frame_time_ms.parse::<f32>().is_ok_and(f32::is_finite),
        ))
        .then(Action::Screenshot("stress_metrics".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("stress_test_metrics"))
        .build()
}
