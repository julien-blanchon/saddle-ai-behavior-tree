use saddle_ai_behavior_tree_example_common as common;

use saddle_ai_behavior_tree::{ActionHandler, BehaviorStatus, BehaviorTreeConfig, TickMode};
use bevy::prelude::*;

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 0.75);

    let (definition, _) = common::basic_definition();
    let definition_id = app
        .world_mut()
        .resource_mut::<saddle_ai_behavior_tree::BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    common::register_condition(
        &mut app,
        "ready",
        saddle_ai_behavior_tree::ConditionHandler::new(|_ctx| true),
    );
    common::register_action(
        &mut app,
        "act",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    for _ in 0..2_048 {
        app.world_mut().spawn(
            saddle_ai_behavior_tree::BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                tick_mode: TickMode::Interval {
                    seconds: 0.016,
                    phase_offset: 0.0,
                },
                restart_on_completion: true,
                ..Default::default()
            }),
        );
    }
    app.add_systems(Update, report_stress);
    app.run();
}

fn report_stress(time: Res<Time>, query: Query<&saddle_ai_behavior_tree::BehaviorTreeInstance>) {
    if (time.elapsed_secs() * 10.0).round() as i32 % 3 == 0 {
        let total_ticks: u64 = query
            .iter()
            .map(|instance| instance.metrics.tick_count)
            .sum();
        info!("agents={} total_ticks={total_ticks}", query.iter().count());
    }
}
