use bevy::prelude::*;
use saddle_ai_behavior_tree::{BehaviorTreeAgent, BehaviorTreeInstance, BehaviorTreeRunState};
use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};

pub fn list_scenarios() -> Vec<&'static str> {
    vec!["smoke_launch", "hot_swap_cycle"]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "hot_swap_cycle" => Some(hot_swap_cycle()),
        _ => None,
    }
}

fn smoke_launch() -> Scenario {
    Scenario::builder("smoke_launch")
        .description("Boot the hot-swap example, verify the agent starts on the patrol tree, and capture the initial overlay.")
        .then(Action::WaitFrames(20))
        .then(assertions::entity_exists::<BehaviorTreeAgent>("swap agent spawned"))
        .then(assertions::resource_satisfies::<crate::SwapState>(
            "agent starts on patrol tree",
            |state| state.is_patrol && state.swap_count == 0,
        ))
        .then(assertions::component_satisfies::<BehaviorTreeInstance>(
            "tree is running on launch",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(Action::Screenshot("hot_swap_patrol".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("smoke_launch"))
        .build()
}

fn hot_swap_cycle() -> Scenario {
    Scenario::builder("hot_swap_cycle")
        .description("Press SPACE twice to swap patrol -> attack -> patrol, verifying the definition id and runtime labels track the active tree.")
        .then(Action::WaitFrames(20))
        .then(assertions::resource_satisfies::<crate::SwapState>(
            "agent starts on patrol tree",
            |state| state.is_patrol && state.swap_count == 0,
        ))
        .then(Action::Screenshot("hot_swap_before".into()))
        .then(Action::PressKey(KeyCode::Space))
        .then(Action::WaitFrames(1))
        .then(Action::ReleaseKey(KeyCode::Space))
        .then(Action::WaitUntil {
            label: "first swap applied".into(),
            condition: Box::new(|world| world.resource::<crate::SwapState>().swap_count >= 1),
            max_frames: 30,
        })
        .then(assertions::resource_satisfies::<crate::SwapState>(
            "first swap switches to attack tree",
            |state| !state.is_patrol && state.swap_count >= 1,
        ))
        .then(assertions::custom(
            "agent definition points at attack tree",
            |world| {
                let state = world.resource::<crate::SwapState>();
                world
                    .get::<BehaviorTreeAgent>(state.entity)
                    .is_some_and(|agent| agent.definition == state.attack_id)
            },
        ))
        .then(Action::Screenshot("hot_swap_attack".into()))
        .then(Action::PressKey(KeyCode::Space))
        .then(Action::WaitFrames(1))
        .then(Action::ReleaseKey(KeyCode::Space))
        .then(Action::WaitUntil {
            label: "second swap applied".into(),
            condition: Box::new(|world| world.resource::<crate::SwapState>().swap_count >= 2),
            max_frames: 30,
        })
        .then(assertions::resource_satisfies::<crate::SwapState>(
            "second swap returns to patrol tree",
            |state| state.is_patrol && state.swap_count >= 2,
        ))
        .then(assertions::custom(
            "agent definition points back at patrol tree",
            |world| {
                let state = world.resource::<crate::SwapState>();
                world
                    .get::<BehaviorTreeAgent>(state.entity)
                    .is_some_and(|agent| agent.definition == state.patrol_id)
            },
        ))
        .then(assertions::component_satisfies::<BehaviorTreeInstance>(
            "tree remains running after repeated swaps",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(Action::Screenshot("hot_swap_patrol_returned".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("hot_swap_cycle"))
        .build()
}
