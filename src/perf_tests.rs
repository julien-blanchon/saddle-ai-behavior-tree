use bevy::prelude::*;

use crate::{
    BehaviorTreePlugin,
    builder::BehaviorTreeBuilder,
    components::BehaviorTreeAgent,
    handlers::ActionHandler,
    nodes::BehaviorStatus,
    resources::{BehaviorTreeHandlers, BehaviorTreeLibrary},
    runtime::{BehaviorTreeConfig, TickMode},
};

#[test]
fn simple_mass_agent_smoke_runs() {
    let mut builder = BehaviorTreeBuilder::new("mass");
    let root = builder.action("Idle", "idle");
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "idle",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );

    for _ in 0..512 {
        app.world_mut()
            .spawn(
                BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                    tick_mode: TickMode::Interval {
                        seconds: 0.016,
                        phase_offset: 0.0,
                    },
                    restart_on_completion: true,
                    ..Default::default()
                }),
            );
    }

    for _ in 0..5 {
        app.update();
    }

    let total_ticks: u64 = {
        let world = app.world_mut();
        let mut query = world.query::<&crate::runtime::BehaviorTreeInstance>();
        query
            .iter(world)
            .map(|instance| instance.metrics.tick_count)
            .sum()
    };
    assert!(total_ticks > 0);
}
