use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorTreeAgent, BehaviorTreeBlackboard, BehaviorTreeBuilder,
    BehaviorTreeConfig, BehaviorTreeHandlers, BehaviorTreeInstance, BehaviorTreeLibrary,
    BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems, BlackboardKeyDirection,
    BlackboardKeyId, BlackboardValueChanged, BranchAborted, ConditionHandler, NodeFinished,
    NodeStarted, ServiceHandler, TickMode, TreeCompleted,
};
use saddle_pane::prelude::*;

#[derive(Resource)]
pub struct ExitTimer(pub Timer);

#[derive(Resource, Clone, Pane)]
#[pane(title = "Behavior Tree Demo")]
pub struct BehaviorTreeExamplePane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    pub time_scale: f32,
    pub manual_tick_mode: bool,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    pub interval_seconds: f32,
    pub ready: bool,
    pub alert: bool,
    pub target_visible: bool,
}

impl Default for BehaviorTreeExamplePane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            manual_tick_mode: false,
            interval_seconds: 0.2,
            ready: true,
            alert: false,
            target_visible: false,
        }
    }
}

pub fn pane_plugins() -> (
    bevy_flair::FlairPlugin,
    bevy_input_focus::InputDispatchPlugin,
    bevy_ui_widgets::UiWidgetsPlugins,
    bevy_input_focus::tab_navigation::TabNavigationPlugin,
    saddle_pane::PanePlugin,
) {
    (
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        saddle_pane::PanePlugin,
    )
}

pub fn headless_app() -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree demo".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(pane_plugins());
    app.register_pane::<BehaviorTreeExamplePane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane_to_runtime,
            decorate_agents,
            update_agent_visuals,
            drift_agents,
        ),
    );
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

fn setup_scene(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Backdrop"),
        Sprite::from_color(Color::srgb(0.07, 0.09, 0.13), Vec2::new(1600.0, 900.0)),
        Transform::from_xyz(0.0, 0.0, -30.0),
    ));
    commands.spawn((
        Name::new("Combat Lane"),
        Sprite::from_color(Color::srgba(0.54, 0.18, 0.14, 0.24), Vec2::new(1160.0, 140.0)),
        Transform::from_xyz(0.0, 120.0, -20.0),
    ));
    commands.spawn((
        Name::new("Patrol Lane"),
        Sprite::from_color(Color::srgba(0.17, 0.39, 0.61, 0.24), Vec2::new(1160.0, 140.0)),
        Transform::from_xyz(0.0, -120.0, -20.0),
    ));
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
        .spawn((
            BehaviorTreeAgent::new(definition_id).with_config(config),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    (entity, definition_id)
}

fn decorate_agents(
    mut commands: Commands,
    agents: Query<(Entity, Option<&Sprite>), Added<BehaviorTreeAgent>>,
) {
    for (entity, sprite) in &agents {
        if sprite.is_some() {
            continue;
        }

        commands.entity(entity).insert(Sprite::from_color(
            Color::srgb(0.24, 0.63, 0.92),
            Vec2::new(64.0, 64.0),
        ));
    }
}

fn sync_pane_to_runtime(
    pane: Res<BehaviorTreeExamplePane>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut agents: Query<&mut BehaviorTreeAgent>,
    mut blackboards: Query<&mut BehaviorTreeBlackboard>,
) {
    if !pane.is_changed() {
        return;
    }

    virtual_time.set_relative_speed(pane.time_scale.max(0.1));

    for mut agent in &mut agents {
        agent.config.tick_mode = if pane.manual_tick_mode {
            TickMode::Manual
        } else {
            TickMode::Interval {
                seconds: pane.interval_seconds.max(0.01),
                phase_offset: 0.0,
            }
        };
    }

    for mut blackboard in &mut blackboards {
        for (key_name, value) in [
            ("ready", pane.ready),
            ("alert", pane.alert),
            ("target_visible", pane.target_visible),
        ] {
            if let Some(key) = blackboard.schema.find_key(key_name) {
                let _ = blackboard.set(key, value);
            }
        }
    }
}

fn update_agent_visuals(
    instances: Query<(Entity, &BehaviorTreeInstance, Option<&BehaviorTreeBlackboard>)>,
    mut sprites: Query<&mut Sprite, With<BehaviorTreeAgent>>,
) {
    for (entity, instance, blackboard) in &instances {
        let Ok(mut sprite) = sprites.get_mut(entity) else {
            continue;
        };

        let alerted = blackboard
            .and_then(|board| board.schema.find_key("alert").and_then(|key| board.get_bool(key)))
            .unwrap_or(false);

        sprite.color = match (instance.status.clone(), alerted) {
            (_, true) => Color::srgb(0.93, 0.38, 0.22),
            (BehaviorTreeRunState::Running, false) => Color::srgb(0.24, 0.63, 0.92),
            (BehaviorTreeRunState::Success, false) => Color::srgb(0.24, 0.84, 0.44),
            (BehaviorTreeRunState::Failure, false) => Color::srgb(0.85, 0.24, 0.28),
            _ => Color::srgb(0.72, 0.76, 0.84),
        };
    }
}

fn drift_agents(time: Res<Time>, mut agents: Query<&mut Transform, With<BehaviorTreeAgent>>) {
    let now = time.elapsed_secs();
    for (index, mut transform) in agents.iter_mut().enumerate() {
        let lane = if index % 2 == 0 { 120.0 } else { -120.0 };
        transform.translation.x = (now * 1.2 + index as f32).sin() * 260.0;
        transform.translation.y = lane + (now * 2.4 + index as f32).cos() * 16.0;
    }
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

pub fn basic_definition() -> (
    saddle_ai_behavior_tree::BehaviorTreeDefinition,
    BlackboardKeyId,
) {
    let mut builder = BehaviorTreeBuilder::new("basic");
    let ready = builder.bool_key("ready", BlackboardKeyDirection::Input, false, Some(true));
    let condition = builder.condition_with_watch_keys("Ready", "ready", [ready]);
    let action = builder.action("Act", "act");
    let root = builder.sequence("Root", [condition, action]);
    builder.set_root(root);
    (builder.build().unwrap(), ready)
}
