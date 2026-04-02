use saddle_ai_behavior_tree_example_common as common;

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeBuilder, BehaviorTreeConfig, BehaviorTreeLibrary,
    TreeResetRequested,
};

#[derive(Resource)]
struct SwapState {
    entity: Entity,
    second: saddle_ai_behavior_tree::BehaviorTreeDefinitionId,
    swapped: bool,
}

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 1.0);
    common::add_logging_systems(&mut app);

    let mut first = BehaviorTreeBuilder::new("first");
    let first_root = first.action("Patrol", "patrol");
    first.set_root(first_root);
    let mut second = BehaviorTreeBuilder::new("second");
    let second_root = second.action("Attack", "attack");
    second.set_root(second_root);
    let first_definition = first.build().unwrap();
    let second_definition = second.build().unwrap();
    let first_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(first_definition)
        .unwrap();
    let second_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(second_definition)
        .unwrap();
    let entity = app
        .world_mut()
        .spawn(
            saddle_ai_behavior_tree::BehaviorTreeAgent::new(first_id).with_config(
                BehaviorTreeConfig {
                    emit_lifecycle_messages: true,
                    ..Default::default()
                },
            ),
        )
        .id();
    common::register_action(
        &mut app,
        "patrol",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    common::register_action(
        &mut app,
        "attack",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    app.insert_resource(SwapState {
        entity,
        second: second_id,
        swapped: false,
    });
    app.add_systems(Update, swap_definition);

    app.run();
}

fn swap_definition(
    time: Res<Time>,
    mut state: ResMut<SwapState>,
    mut agents: Query<&mut saddle_ai_behavior_tree::BehaviorTreeAgent>,
    mut resets: MessageWriter<TreeResetRequested>,
) {
    if !state.swapped && time.elapsed_secs() > 0.3 {
        let mut agent = agents.get_mut(state.entity).unwrap();
        agent.definition = state.second;
        resets.write(TreeResetRequested::new(state.entity, "definition swapped"));
        state.swapped = true;
    }
}
