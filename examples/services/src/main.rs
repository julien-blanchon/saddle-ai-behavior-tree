use saddle_ai_behavior_tree_example_common as common;

use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeBuilder, BehaviorTreeConfig, ServiceBinding,
    ServiceHandler,
};
use bevy::prelude::*;

#[derive(Resource, Default)]
struct Pulse(u32);

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 0.75);
    common::add_logging_systems(&mut app);
    app.init_resource::<Pulse>();

    let mut builder = BehaviorTreeBuilder::new("services");
    let idle = builder.action("Idle", "idle");
    let root = builder.repeater("Root", Some(4), idle);
    builder.add_service(root, ServiceBinding::new("Sense", "sense", 0.1));
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let _ = common::register_tree(
        &mut app,
        definition,
        BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            ..Default::default()
        },
    );
    common::register_action(
        &mut app,
        "idle",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    common::register_service(
        &mut app,
        "sense",
        ServiceHandler::new(|ctx| {
            let mut pulse = ctx.world.resource_mut::<Pulse>();
            pulse.0 += 1;
            info!("service tick {}", pulse.0);
        }),
    );

    app.run();
}
