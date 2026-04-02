use bevy::gizmos::prelude::AppGizmoBuilder;
use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeDebugRender, BehaviorTreeHandlers, BehaviorTreeLibrary, BehaviorTreePlugin,
    BlackboardKeyDirection, NodeId,
};

#[derive(Resource)]
struct DemoKey(saddle_ai_behavior_tree::BlackboardKeyId);

#[derive(Resource)]
struct DemoAgent(Entity);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_gizmo_group::<saddle_ai_behavior_tree::BehaviorTreeDebugGizmos>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.add_systems(Startup, setup);
    app.add_systems(Update, (update_overlay, tick_target, exit_after));
    app.run();
}

fn setup(
    mut commands: Commands,
    mut library: ResMut<BehaviorTreeLibrary>,
    mut handlers: ResMut<BehaviorTreeHandlers>,
) {
    commands.spawn(Camera2d);
    let mut builder = BehaviorTreeBuilder::new("debug_overlay");
    let alert = builder.bool_key("alert", BlackboardKeyDirection::Input, false, Some(false));
    let alert_condition = builder.condition_with_watch_keys("Alert", "alert", [alert]);
    let respond = builder.action("Respond", "respond");
    let alert_branch = builder.sequence("AlertBranch", [alert_condition, respond]);
    let idle = builder.action("Idle", "idle");
    let root = builder.reactive_selector(
        "Root",
        saddle_ai_behavior_tree::AbortPolicy::LowerPriority,
        [alert_branch, idle],
    );
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let definition_id = library.register(definition).unwrap();
    handlers.register_condition(
        "alert",
        saddle_ai_behavior_tree::ConditionHandler::new(move |ctx| {
            ctx.blackboard.get_bool(alert).unwrap_or(false)
        }),
    );
    handlers.register_action(
        "respond",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    handlers.register_action(
        "idle",
        ActionHandler::stateful(
            |_ctx| BehaviorStatus::Running,
            |_ctx| BehaviorStatus::Running,
            |_ctx| {},
        ),
    );
    let entity = commands
        .spawn((
            Name::new("Debug Agent"),
            BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                emit_lifecycle_messages: true,
                ..Default::default()
            }),
            BehaviorTreeDebugRender::default(),
        ))
        .id();
    commands.insert_resource(DemoKey(alert));
    commands.insert_resource(DemoAgent(entity));
    commands.spawn((
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: px(12.0),
            left: px(12.0),
            ..default()
        },
    ));
}

fn update_overlay(
    agent: Res<DemoAgent>,
    key: Res<DemoKey>,
    query: Query<
        (
            &saddle_ai_behavior_tree::BehaviorTreeInstance,
            &saddle_ai_behavior_tree::BehaviorTreeBlackboard,
        ),
        With<BehaviorTreeAgent>,
    >,
    mut texts: Query<&mut Text>,
) {
    let Ok((instance, blackboard)) = query.get(agent.0) else {
        return;
    };
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let path: Vec<String> = instance
        .active_path
        .iter()
        .map(|NodeId(id)| id.to_string())
        .collect();
    text.0 = format!(
        "active path: {:?}\nstatus: {:?}\nalert: {:?}\nlast abort: {}",
        path,
        instance.status,
        blackboard.get_bool(key.0),
        instance.last_abort_reason
    );
}

fn tick_target(
    time: Res<Time>,
    agent: Res<DemoAgent>,
    key: Res<DemoKey>,
    mut query: Query<&mut saddle_ai_behavior_tree::BehaviorTreeBlackboard>,
) {
    if let Ok(mut blackboard) = query.get_mut(agent.0) {
        let alert = (time.elapsed_secs() * 2.0).sin() > 0.4;
        let _ = blackboard.set(key.0, alert);
    }
}

fn exit_after(time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs() > 2.5 {
        exit.write(AppExit::Success);
    }
}
