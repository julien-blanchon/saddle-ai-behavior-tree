//! Behavior tree — reactive abort example
//!
//! Demonstrates reactive selector with `AbortPolicy::LowerPriority`.
//!
//! The tree has two branches under a reactive selector:
//!   1. **AttackBranch** (higher priority): condition "CanAttack" + action "Attack"
//!   2. **Patrol** (lower priority): long-running patrol action
//!
//! When "CanAttack" becomes true, the reactive selector aborts the running
//! Patrol action and switches to the AttackBranch. Toggle `target_visible`
//! in the pane to trigger this behavior.

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    AbortPolicy, ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBlackboard,
    BehaviorTreeBuilder, BehaviorTreeConfig, BehaviorTreeHandlers, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems,
    BlackboardKeyDirection, ConditionHandler, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Clone, Pane)]
#[pane(title = "Reactive Abort")]
struct AbortPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    pub target_visible: bool,
    pub auto_flip: bool,
    #[pane(slider, min = 0.1, max = 3.0, step = 0.05)]
    pub auto_flip_interval: f32,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    abort_count: String,
}

impl Default for AbortPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            target_visible: false,
            auto_flip: true,
            auto_flip_interval: 0.5,
            status: "Idle".into(),
            abort_count: "0".into(),
        }
    }
}

#[derive(Resource)]
struct AbortState {
    entity: Entity,
    visible_key: saddle_ai_behavior_tree::BlackboardKeyId,
    flip_timer: Timer,
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / reactive_abort".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        PanePlugin,
    ));
    app.register_pane::<AbortPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

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

    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    let entity = app
        .world_mut()
        .spawn((
            Name::new("AbortAgent"),
            BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                emit_lifecycle_messages: true,
                restart_on_completion: true,
                trace_capacity: 64,
                ..Default::default()
            }),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Sprite::from_color(Color::srgb(0.24, 0.63, 0.92), Vec2::new(64.0, 64.0)),
        ))
        .id();

    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition(
            "can_attack",
            ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(visible).unwrap_or(false)),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "attack",
            ActionHandler::instant(|_ctx| {
                info!("Attack!");
                BehaviorStatus::Success
            }),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "patrol",
            ActionHandler::stateful(
                |_ctx| {
                    info!("Patrol started");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |_ctx| info!("Patrol ABORTED!"),
            ),
        );

    app.insert_resource(AbortState {
        entity,
        visible_key: visible,
        flip_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            auto_flip_visibility,
            update_monitors,
            update_sprite,
            common::update_tree_overlay.after(BehaviorTreeSystems::Cleanup),
        ),
    );
    common::add_logging_systems(&mut app);

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Backdrop"),
        Sprite::from_color(Color::srgb(0.07, 0.09, 0.13), Vec2::new(1600.0, 900.0)),
        Transform::from_xyz(0.0, 0.0, -30.0),
    ));

    common::spawn_tree_overlay(&mut commands);
    common::spawn_instructions(
        &mut commands,
        "Toggle 'target_visible' in the pane to trigger reactive abort.\n\
         When visible=true, the higher-priority AttackBranch activates and\n\
         aborts the running Patrol. Watch the abort count increase.\n\
         Use 'auto_flip' to automatically toggle visibility on an interval.",
    );
}

fn sync_pane(
    pane: Res<AbortPane>,
    mut state: ResMut<AbortState>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut agents: Query<&mut BehaviorTreeAgent>,
    mut blackboards: Query<&mut BehaviorTreeBlackboard>,
) {
    if !pane.is_changed() {
        return;
    }
    virtual_time.set_relative_speed(pane.time_scale.max(0.1));

    for mut agent in &mut agents {
        agent.config.tick_mode = TickMode::Interval {
            seconds: pane.interval_seconds.max(0.01),
            phase_offset: 0.0,
        };
    }

    state
        .flip_timer
        .set_duration(std::time::Duration::from_secs_f32(
            pane.auto_flip_interval.max(0.05),
        ));

    if !pane.auto_flip {
        if let Ok(mut bb) = blackboards.get_mut(state.entity) {
            let _ = bb.set(state.visible_key, pane.target_visible);
        }
    }
}

fn auto_flip_visibility(
    time: Res<Time>,
    pane: Res<AbortPane>,
    mut state: ResMut<AbortState>,
    mut blackboards: Query<&mut BehaviorTreeBlackboard>,
) {
    if !pane.auto_flip {
        return;
    }
    if state.flip_timer.tick(time.delta()).just_finished() {
        if let Ok(mut bb) = blackboards.get_mut(state.entity) {
            let current = bb.get_bool(state.visible_key).unwrap_or(false);
            let _ = bb.set(state.visible_key, !current);
        }
    }
}

fn update_monitors(
    state: Res<AbortState>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<AbortPane>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
        pane.abort_count = format!("{}", instance.metrics.abort_count);
    }
}

fn update_sprite(
    state: Res<AbortState>,
    instances: Query<&BehaviorTreeInstance>,
    mut sprites: Query<&mut Sprite>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        if let Ok(mut sprite) = sprites.get_mut(state.entity) {
            sprite.color = match &instance.status {
                BehaviorTreeRunState::Running => Color::srgb(0.24, 0.63, 0.92),
                BehaviorTreeRunState::Success => Color::srgb(0.24, 0.84, 0.44),
                BehaviorTreeRunState::Failure => Color::srgb(0.85, 0.24, 0.28),
                _ => Color::srgb(0.72, 0.76, 0.84),
            };
        }
    }
}
