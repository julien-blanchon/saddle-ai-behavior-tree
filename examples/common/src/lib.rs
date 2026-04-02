use std::time::Duration;

use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeHandlers, BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeSystems,
    BlackboardKeyDirection, BlackboardKeyId, BlackboardValueChanged, BranchAborted,
    ConditionHandler, NodeFinished, NodeStarted, ServiceHandler, TreeCompleted,
};
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;

#[derive(Resource)]
pub struct ExitTimer(pub Timer);

pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))));
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app
}

pub fn install_exit_timer(app: &mut App, seconds: f32) {
    app.insert_resource(ExitTimer(Timer::from_seconds(seconds, TimerMode::Once)));
    app.add_systems(Update, exit_after_timer);
}

fn exit_after_timer(
    time: Res<Time>,
    mut timer: ResMut<ExitTimer>,
    mut exit: MessageWriter<AppExit>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        exit.write(AppExit::Success);
    }
}

pub fn register_tree(
    app: &mut App,
    definition: saddle_ai_behavior_tree::BehaviorTreeDefinition,
    config: BehaviorTreeConfig,
) -> (Entity, saddle_ai_behavior_tree::BehaviorTreeDefinitionId) {
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    let entity = app
        .world_mut()
        .spawn(BehaviorTreeAgent::new(definition_id).with_config(config))
        .id();
    (entity, definition_id)
}

pub fn add_logging_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            log_started,
            log_finished,
            log_completed,
            log_aborts,
            log_blackboard_changes,
        )
            .after(BehaviorTreeSystems::Apply),
    );
}

fn log_started(mut reader: MessageReader<NodeStarted>) {
    for message in reader.read() {
        info!("start: {}", message.path);
    }
}

fn log_finished(mut reader: MessageReader<NodeFinished>) {
    for message in reader.read() {
        info!("finish: {} -> {:?}", message.path, message.status);
    }
}

fn log_completed(mut reader: MessageReader<TreeCompleted>) {
    for message in reader.read() {
        info!(
            "tree completed: {:?} on {:?}",
            message.status, message.entity
        );
    }
}

fn log_aborts(mut reader: MessageReader<BranchAborted>) {
    for message in reader.read() {
        info!("abort: {} ({})", message.path, message.reason);
    }
}

fn log_blackboard_changes(mut reader: MessageReader<BlackboardValueChanged>) {
    for message in reader.read() {
        info!("blackboard: {} -> {:?}", message.name, message.new_value);
    }
}

pub fn register_action(app: &mut App, name: &str, handler: ActionHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(name, handler);
}

pub fn register_condition(app: &mut App, name: &str, handler: ConditionHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition(name, handler);
}

pub fn register_service(app: &mut App, name: &str, handler: ServiceHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_service(name, handler);
}

pub fn basic_definition() -> (saddle_ai_behavior_tree::BehaviorTreeDefinition, BlackboardKeyId) {
    let mut builder = BehaviorTreeBuilder::new("basic");
    let ready = builder.bool_key("ready", BlackboardKeyDirection::Input, false, Some(true));
    let condition = builder.condition_with_watch_keys("Ready", "ready", [ready]);
    let action = builder.action("Act", "act");
    let root = builder.sequence("Root", [condition, action]);
    builder.set_root(root);
    (builder.build().unwrap(), ready)
}
