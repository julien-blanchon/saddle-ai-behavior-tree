use saddle_ai_behavior_tree_example_common as common;

use saddle_ai_behavior_tree::{
    ActionHandler, ActionResolution, ActionTicket, BehaviorStatus, BehaviorTreeBuilder,
    BehaviorTreeConfig,
};
use bevy::prelude::*;

#[derive(Resource, Default)]
struct PendingTicket(Option<ActionTicket>);

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 1.0);
    common::add_logging_systems(&mut app);
    app.init_resource::<PendingTicket>();

    let mut builder = BehaviorTreeBuilder::new("async");
    let root = builder.action("AsyncWork", "async_work");
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let (entity, _) = common::register_tree(
        &mut app,
        definition,
        BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            ..Default::default()
        },
    );
    common::register_action(
        &mut app,
        "async_work",
        ActionHandler::stateful(
            |ctx| {
                let ticket = ctx.request_async_ticket();
                ctx.world.resource_mut::<PendingTicket>().0 = Some(ticket);
                BehaviorStatus::Running
            },
            |ctx| {
                ctx.take_async_resolution()
                    .unwrap_or(BehaviorStatus::Running)
            },
            |_ctx| {},
        ),
    );
    app.add_systems(
        Update,
        move |mut pending: ResMut<PendingTicket>,
              time: Res<Time>,
              mut exit: MessageWriter<ActionResolution>| {
            if time.elapsed_secs() > 0.35
                && let Some(ticket) = pending.0.take()
            {
                exit.write(ActionResolution::new(
                    entity,
                    ticket,
                    BehaviorStatus::Success,
                ));
            }
        },
    );

    app.run();
}
