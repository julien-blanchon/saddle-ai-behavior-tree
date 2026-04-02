use saddle_ai_behavior_tree_example_common as common;

use saddle_ai_behavior_tree::{
    AbortPolicy, ActionHandler, BehaviorStatus, BehaviorTreeBuilder, BehaviorTreeConfig,
    BlackboardKeyDirection, ConditionHandler,
};
use bevy::prelude::*;

#[derive(Resource, Default)]
struct FlipTimer(Timer);

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 1.5);
    common::add_logging_systems(&mut app);
    app.insert_resource(FlipTimer(Timer::from_seconds(0.35, TimerMode::Repeating)));

    let mut builder = BehaviorTreeBuilder::new("reactive_abort");
    let visible = builder.bool_key("visible", BlackboardKeyDirection::Input, false, Some(false));
    let can_attack = builder.condition_with_watch_keys("CanAttack", "can_attack", [visible]);
    let attack = builder.action("Attack", "attack");
    let patrol = builder.action("Patrol", "patrol");
    let attack_branch = builder.sequence("AttackBranch", [can_attack, attack]);
    let root =
        builder.reactive_selector("Root", AbortPolicy::LowerPriority, [attack_branch, patrol]);
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
    common::register_condition(
        &mut app,
        "can_attack",
        ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(visible).unwrap_or(false)),
    );
    common::register_action(
        &mut app,
        "attack",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    common::register_action(
        &mut app,
        "patrol",
        ActionHandler::stateful(
            |_ctx| BehaviorStatus::Running,
            |_ctx| BehaviorStatus::Running,
            |_ctx| info!("patrol aborted"),
        ),
    );
    app.add_systems(
        Update,
        move |time: Res<Time>,
              mut timer: ResMut<FlipTimer>,
              mut blackboards: Query<&mut saddle_ai_behavior_tree::BehaviorTreeBlackboard>| {
            if timer.0.tick(time.delta()).just_finished() {
                let mut blackboard = blackboards.get_mut(entity).unwrap();
                let next = !blackboard.get_bool(visible).unwrap_or(false);
                blackboard.set(visible, next).unwrap();
            }
        },
    );

    app.run();
}
