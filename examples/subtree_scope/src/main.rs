use saddle_ai_behavior_tree_example_common as common;

use saddle_ai_behavior_tree::{
    AbortPolicy, ActionHandler, BehaviorStatus, BehaviorTreeBuilder, BehaviorTreeConfig,
    BlackboardCondition, BlackboardKeyDirection, SubtreeRemap,
};
use bevy::prelude::*;

fn main() {
    let mut app = common::headless_app();
    common::install_exit_timer(&mut app, 0.5);
    common::add_logging_systems(&mut app);

    let mut template = BehaviorTreeBuilder::new("worker");
    let ready = template.bool_key("ready", BlackboardKeyDirection::Input, true, Some(false));
    let _result = template.text_key("result", BlackboardKeyDirection::Output, false, None);
    let work = template.action("Work", "work");
    let root = template.blackboard_condition(
        "Ready",
        ready,
        BlackboardCondition::IsTrue,
        AbortPolicy::Both,
        work,
    );
    template.set_root(root);
    let template = template.build().unwrap();

    let mut builder = BehaviorTreeBuilder::new("subtree_scope");
    let parent_ready = builder.bool_key("ready", BlackboardKeyDirection::Input, true, Some(true));
    let parent_result = builder.text_key("result", BlackboardKeyDirection::Output, false, None);
    let subtree = builder
        .inline_subtree(
            "job_loop",
            &template,
            [
                SubtreeRemap::new("ready", parent_ready),
                SubtreeRemap::new("result", parent_result),
            ],
        )
        .unwrap();
    builder.set_root(subtree);
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
        "work",
        ActionHandler::instant(move |ctx| {
            ctx.blackboard
                .set(parent_result, "subtree completed")
                .unwrap();
            BehaviorStatus::Success
        }),
    );
    app.add_systems(
        Update,
        move |query: Query<&saddle_ai_behavior_tree::BehaviorTreeBlackboard>| {
            if let Ok(blackboard) = query.get(entity)
                && let Some(result) = blackboard.get_text(parent_result)
            {
                info!("result: {result}");
            }
        },
    );

    app.run();
}
