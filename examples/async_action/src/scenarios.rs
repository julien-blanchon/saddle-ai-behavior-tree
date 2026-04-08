use bevy::prelude::*;
use saddle_ai_behavior_tree::{BehaviorTreeAgent, BehaviorTreeInstance, BehaviorTreeRunState};
use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["smoke_launch", "async_action_resolve_cycle"]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "async_action_resolve_cycle" => Some(async_action_resolve_cycle()),
        _ => None,
    }
}

fn wait_for_pending_ticket(label: &str, max_frames: u32) -> Action {
    Action::WaitUntil {
        label: label.into(),
        condition: Box::new(|world| world.resource::<crate::PendingTicket>().0.is_some()),
        max_frames,
    }
}

fn smoke_launch() -> Scenario {
    Scenario::builder("smoke_launch")
        .description("Boot the async action example, wait for the ticket-backed action to enter its pending state, and capture the initial waiting view.")
        .then(wait_for_pending_ticket("pending async ticket issued", 90))
        .then(assertions::entity_exists::<BehaviorTreeAgent>("async agent spawned"))
        .then(assertions::resource_satisfies::<crate::PendingTicket>(
            "async ticket is pending",
            |pending| pending.0.is_some(),
        ))
        .then(assertions::component_satisfies::<BehaviorTreeInstance>(
            "tree is running while awaiting external completion",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(Action::Screenshot("async_waiting".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("smoke_launch"))
        .build()
}

fn async_action_resolve_cycle() -> Scenario {
    Scenario::builder("async_action_resolve_cycle")
        .description("Wait for the async ticket, resolve it with SPACE, then verify the tree restarts and issues a fresh pending ticket.")
        .then(wait_for_pending_ticket("pending async ticket issued", 90))
        .then(assertions::resource_satisfies::<crate::AsyncState>(
            "resolve count starts at zero",
            |state| state.resolve_count == 0,
        ))
        .then(Action::Screenshot("async_before_resolve".into()))
        .then(Action::PressKey(KeyCode::Space))
        .then(Action::WaitFrames(1))
        .then(Action::ReleaseKey(KeyCode::Space))
        .then(Action::WaitUntil {
            label: "resolve count increments".into(),
            condition: Box::new(|world| world.resource::<crate::AsyncState>().resolve_count >= 1),
            max_frames: 30,
        })
        .then(assertions::resource_satisfies::<crate::AsyncState>(
            "resolve count increments after SPACE",
            |state| state.resolve_count >= 1,
        ))
        .then(Action::WaitFrames(20))
        .then(assertions::component_satisfies::<BehaviorTreeInstance>(
            "tree resumes running after restart",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(assertions::resource_satisfies::<crate::PendingTicket>(
            "new async ticket is pending after restart",
            |pending| pending.0.is_some(),
        ))
        .then(Action::Screenshot("async_resolved_and_restarted".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("async_action_resolve_cycle"))
        .build()
}
