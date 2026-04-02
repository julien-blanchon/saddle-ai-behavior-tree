use saddle_ai_behavior_tree_example_common as common;

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeConfig, ConditionHandler,
};

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 0.5);
    common::add_logging_systems(&mut app);

    let (definition, ready_key) = common::basic_definition();
    let (_entity, _) = common::register_tree(
        &mut app,
        definition,
        BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            ..Default::default()
        },
    );
    common::register_condition(
        &mut app,
        "ready",
        ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(ready_key).unwrap_or(false)),
    );
    common::register_action(
        &mut app,
        "act",
        ActionHandler::instant(|ctx| {
            info!("entity {:?} performed the basic action", ctx.entity);
            BehaviorStatus::Success
        }),
    );

    app.run();
}
