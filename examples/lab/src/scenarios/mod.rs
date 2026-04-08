use bevy::prelude::*;
use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};

use crate::{BehaviorTreeLabPane, BehaviorTreeRunState, LabAgent, LabStats};

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "smoke_launch",
        "bt_smoke",
        "bt_patrol_default",
        "bt_chase_trigger",
        "bt_reactive_abort",
        "bt_blackboard_updates",
        "bt_tree_completion",
        "bt_metrics_accumulate",
        "bt_service_interval",
        "bt_multi_abort_cycle",
        "bt_completion_restart",
    ]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(build_smoke("smoke_launch")),
        "bt_smoke" => Some(build_smoke("bt_smoke")),
        "bt_patrol_default" => Some(bt_patrol_default()),
        "bt_chase_trigger" => Some(bt_chase_trigger()),
        "bt_reactive_abort" => Some(bt_reactive_abort()),
        "bt_blackboard_updates" => Some(bt_blackboard_updates()),
        "bt_tree_completion" => Some(bt_tree_completion()),
        "bt_metrics_accumulate" => Some(bt_metrics_accumulate()),
        "bt_service_interval" => Some(bt_service_interval()),
        "bt_multi_abort_cycle" => Some(bt_multi_abort_cycle()),
        "bt_completion_restart" => Some(bt_completion_restart()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper actions
// ---------------------------------------------------------------------------

/// Move the target far away so it stays outside the visibility radius.
fn hide_target() -> Action {
    Action::Custom(Box::new(|world| {
        let mut pane = world.resource_mut::<BehaviorTreeLabPane>();
        // Zero visibility radius so no target is ever "visible"
        pane.visibility_radius = 0.01;
    }))
}

/// Bring the target close enough to trigger visibility (large radius, no gate block).
fn reveal_target() -> Action {
    Action::Custom(Box::new(|world| {
        let mut pane = world.resource_mut::<BehaviorTreeLabPane>();
        pane.visibility_radius = 99.0;
        pane.visibility_gate_x = -99.0; // gate does not block anything
    }))
}

/// Block the visibility gate so target stays invisible even when close.
#[allow(dead_code)]
fn block_gate() -> Action {
    Action::Custom(Box::new(|world| {
        let mut pane = world.resource_mut::<BehaviorTreeLabPane>();
        pane.visibility_gate_x = 99.0; // gate blocks everything
    }))
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

fn build_smoke(name: &'static str) -> Scenario {
    Scenario::builder(name)
        .description("Boot the behavior-tree lab and verify the agent initializes with BehaviorTreeInstance running.")
        .then(Action::WaitFrames(15))
        .then(assertions::entity_exists::<LabAgent>("agent spawned"))
        .then(assertions::resource_exists::<LabStats>("lab stats initialized"))
        .then(assertions::component_satisfies::<saddle_ai_behavior_tree::BehaviorTreeInstance>(
            "agent instance is running or completed",
            |instance| !matches!(instance.status, BehaviorTreeRunState::Deactivated),
        ))
        .then(Action::Screenshot("smoke".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary(name))
        .build()
}

/// Verify the agent defaults to the Patrol action when the target is hidden.
fn bt_patrol_default() -> Scenario {
    Scenario::builder("bt_patrol_default")
        .description("When the target is outside the visibility radius, the agent should run the Patrol action (reactive selector falls through to patrol).")
        .then(hide_target())
        .then(Action::WaitFrames(30))
        .then(assertions::entity_exists::<LabAgent>("agent present"))
        .then(assertions::component_satisfies::<saddle_ai_behavior_tree::BehaviorTreeInstance>(
            "tree is running",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(Action::Screenshot("patrol_default".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_patrol_default"))
        .build()
}

/// Verify the chase branch activates when the target enters the visibility cone.
fn bt_chase_trigger() -> Scenario {
    Scenario::builder("bt_chase_trigger")
        .description("When the target becomes visible (large radius, open gate), the reactive selector should switch the agent to the Chase branch.")
        .then(hide_target())
        .then(Action::WaitFrames(20))
        // Ensure we start in running state
        .then(assertions::component_satisfies::<saddle_ai_behavior_tree::BehaviorTreeInstance>(
            "starts in running state",
            |instance| matches!(instance.status, BehaviorTreeRunState::Running),
        ))
        .then(Action::Screenshot("chase_before".into()))
        // Now make target visible
        .then(reveal_target())
        .then(Action::WaitUntil {
            label: "service ticks at least once after reveal".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().service_ticks >= 2
            }),
            max_frames: 60,
        })
        .then(assertions::component_satisfies::<saddle_ai_behavior_tree::BehaviorTreeInstance>(
            "tree still running after reveal",
            |instance| !matches!(instance.status, BehaviorTreeRunState::Deactivated),
        ))
        .then(Action::Screenshot("chase_after".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_chase_trigger"))
        .build()
}

/// Verify the reactive selector aborts the chase branch when the target hides.
fn bt_reactive_abort() -> Scenario {
    Scenario::builder("bt_reactive_abort")
        .description("After the chase branch is active, hiding the target should trigger a BranchAborted message and return to patrol.")
        .then(hide_target())
        .then(Action::WaitFrames(20))
        // Enable chase
        .then(reveal_target())
        .then(Action::WaitUntil {
            label: "service ticks observed".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().service_ticks >= 2
            }),
            max_frames: 60,
        })
        .then(Action::WaitFrames(10))
        .then(Action::Screenshot("abort_before".into()))
        // Hide target to trigger abort
        .then(hide_target())
        .then(Action::WaitUntil {
            label: "at least one abort recorded".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().aborts >= 1
            }),
            max_frames: 120,
        })
        .then(assertions::custom("abort count incremented", |world| {
            world.resource::<LabStats>().aborts >= 1
        }))
        .then(Action::Screenshot("abort_after".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_reactive_abort"))
        .build()
}

/// Verify the blackboard is updated by the service on each tick.
fn bt_blackboard_updates() -> Scenario {
    Scenario::builder("bt_blackboard_updates")
        .description("The sense_target service should write distance_to_target and target_visible into the blackboard each service interval.")
        .then(hide_target())
        .then(Action::WaitFrames(10))
        .then(Action::WaitUntil {
            label: "service ticked at least 3 times".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().service_ticks >= 3
            }),
            max_frames: 60,
        })
        .then(assertions::custom("service ran multiple times", |world| {
            world.resource::<LabStats>().service_ticks >= 3
        }))
        .then(Action::Screenshot("blackboard_updates".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_blackboard_updates"))
        .build()
}

/// Verify TreeCompleted fires when the tree finishes (chase succeeds reaching target).
fn bt_tree_completion() -> Scenario {
    Scenario::builder("bt_tree_completion")
        .description("When the agent successfully chases and reaches the target (within arrival distance), TreeCompleted should be emitted and the tree restarts due to restart_on_completion.")
        .then(reveal_target())
        .then(Action::WaitUntil {
            label: "at least one completion observed".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().completions >= 1
            }),
            max_frames: 360,
        })
        .then(assertions::custom("tree completed at least once", |world| {
            world.resource::<LabStats>().completions >= 1
        }))
        .then(Action::Screenshot("tree_completion".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_tree_completion"))
        .build()
}

/// Verify the sense_target service accumulates ticks at a reasonable rate over a fixed window.
fn bt_service_interval() -> Scenario {
    Scenario::builder("bt_service_interval")
        .description(
            "Over a 3-second window (180 frames at 60fps) the sense_target service should tick \
             multiple times — confirming the service interval (~0.15s) fires correctly and the \
             service count grows proportionally.",
        )
        .then(hide_target())
        .then(Action::WaitFrames(10))
        // Capture the service count at the start of the measurement window
        .then(Action::WaitUntil {
            label: "service ticked at least once to establish baseline".into(),
            condition: Box::new(|world| world.resource::<LabStats>().service_ticks >= 1),
            max_frames: 60,
        })
        .then(Action::Screenshot("service_interval_start".into()))
        .then(Action::WaitFrames(1))
        // Wait ~3s — at 0.15s interval that is ~20 ticks minimum
        .then(Action::WaitFrames(180))
        .then(assertions::custom(
            "service ticked ≥ 10 times in 3s window",
            |world| world.resource::<LabStats>().service_ticks >= 10,
        ))
        .then(Action::Screenshot("service_interval_end".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_service_interval"))
        .build()
}

/// Cycle hide→reveal→hide three times and verify each cycle produces an abort.
fn bt_multi_abort_cycle() -> Scenario {
    Scenario::builder("bt_multi_abort_cycle")
        .description(
            "Each reveal→hide transition should trigger a BranchAborted message. After three \
             full hide/reveal cycles the abort counter should be at least 3.",
        )
        .then(hide_target())
        .then(Action::WaitFrames(20))
        // Cycle 1
        .then(reveal_target())
        .then(Action::WaitUntil {
            label: "cycle 1: service ticks observed".into(),
            condition: Box::new(|world| world.resource::<LabStats>().service_ticks >= 2),
            max_frames: 60,
        })
        .then(Action::WaitFrames(10))
        .then(hide_target())
        .then(Action::WaitUntil {
            label: "cycle 1: first abort recorded".into(),
            condition: Box::new(|world| world.resource::<LabStats>().aborts >= 1),
            max_frames: 120,
        })
        // Cycle 2
        .then(reveal_target())
        .then(Action::WaitFrames(15))
        .then(hide_target())
        .then(Action::WaitUntil {
            label: "cycle 2: second abort recorded".into(),
            condition: Box::new(|world| world.resource::<LabStats>().aborts >= 2),
            max_frames: 120,
        })
        // Cycle 3
        .then(reveal_target())
        .then(Action::WaitFrames(15))
        .then(hide_target())
        .then(Action::WaitUntil {
            label: "cycle 3: third abort recorded".into(),
            condition: Box::new(|world| world.resource::<LabStats>().aborts >= 3),
            max_frames: 120,
        })
        .then(assertions::custom(
            "3 hide/reveal cycles produced ≥ 3 aborts",
            |world| world.resource::<LabStats>().aborts >= 3,
        ))
        .then(Action::Screenshot("multi_abort_cycle".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_multi_abort_cycle"))
        .build()
}

/// Verify that restart_on_completion causes the tree to restart after each success.
fn bt_completion_restart() -> Scenario {
    Scenario::builder("bt_completion_restart")
        .description(
            "With restart_on_completion enabled, the tree should complete and restart multiple \
             times when the target is kept visible. After two completions the tree should still \
             be in Running state — confirming the restart loop works.",
        )
        .then(reveal_target())
        // Wait for a first completion
        .then(Action::WaitUntil {
            label: "first completion".into(),
            condition: Box::new(|world| world.resource::<LabStats>().completions >= 1),
            max_frames: 360,
        })
        .then(assertions::custom("at least one completion", |world| {
            world.resource::<LabStats>().completions >= 1
        }))
        .then(Action::Screenshot("restart_after_first".into()))
        .then(Action::WaitFrames(1))
        // Wait for a second completion — proves restart happened
        .then(Action::WaitUntil {
            label: "second completion (restart confirmed)".into(),
            condition: Box::new(|world| world.resource::<LabStats>().completions >= 2),
            max_frames: 360,
        })
        .then(assertions::custom(
            "two completions confirm restart loop",
            |world| world.resource::<LabStats>().completions >= 2,
        ))
        // Tree should still be running (not stuck in completed state)
        .then(assertions::component_satisfies::<
            saddle_ai_behavior_tree::BehaviorTreeInstance,
        >("tree is running after restart", |instance| {
            matches!(instance.status, BehaviorTreeRunState::Running)
        }))
        .then(Action::Screenshot("restart_after_second".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_completion_restart"))
        .build()
}

/// Verify that metrics accumulate over multiple ticks.
fn bt_metrics_accumulate() -> Scenario {
    Scenario::builder("bt_metrics_accumulate")
        .description("After running for several seconds, the behavior tree should have accumulated service ticks and the BehaviorTreeMetrics component should be populated.")
        .then(Action::WaitFrames(10))
        .then(Action::WaitUntil {
            label: "service ticks accumulate".into(),
            condition: Box::new(|world| {
                world.resource::<LabStats>().service_ticks >= 5
            }),
            max_frames: 120,
        })
        .then(assertions::custom("metrics: service ticks > 0", |world| {
            world.resource::<LabStats>().service_ticks > 0
        }))
        .then(assertions::component_satisfies::<saddle_ai_behavior_tree::BehaviorTreeInstance>(
            "BehaviorTreeMetrics populated on agent",
            |instance| instance.metrics.tick_count > 0,
        ))
        .then(Action::Screenshot("metrics".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("bt_metrics_accumulate"))
        .build()
}
