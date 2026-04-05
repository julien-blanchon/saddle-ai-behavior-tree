//! Behavior tree — debug overlay example
//!
//! Showcases the full debug visualization system:
//!
//! - **Tree overlay**: indented tree structure with node status indicators
//! - **Active path breadcrumb**: shows the current execution path
//! - **Blackboard inspector**: live key-value display
//! - **Trace history**: recent node events (start, finish, abort)
//! - **Metrics footer**: tick count, timing, abort count
//! - **Gizmo rendering**: world-space debug gizmos on agent entities
//!
//! Toggle the `alert` flag in the pane to trigger reactive abort behavior
//! and watch the debug overlay update in real-time.

use bevy::gizmos::prelude::AppGizmoBuilder;
use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    AbortPolicy, ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder,
    BehaviorTreeConfig, BehaviorTreeDebugGizmos, BehaviorTreeDebugRender, BehaviorTreeHandlers,
    BehaviorTreeInstance, BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState,
    BehaviorTreeSystems, BlackboardKeyDirection, ConditionHandler, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Clone, Pane)]
#[pane(title = "Debug Overlay")]
struct DebugPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    pub alert: bool,
    pub auto_toggle: bool,
    #[pane(slider, min = 0.3, max = 3.0, step = 0.1)]
    pub toggle_interval: f32,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    abort_count: String,
    #[pane(monitor)]
    tick_count: String,
}

impl Default for DebugPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.15,
            alert: false,
            auto_toggle: true,
            toggle_interval: 1.2,
            status: "Idle".into(),
            abort_count: "0".into(),
            tick_count: "0".into(),
        }
    }
}

#[derive(Resource)]
struct DebugState {
    entity: Entity,
    alert_key: saddle_ai_behavior_tree::BlackboardKeyId,
    toggle_timer: Timer,
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / debug_overlay".into(),
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
    app.register_pane::<DebugPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.init_gizmo_group::<BehaviorTreeDebugGizmos>();

    // Build a reactive selector tree:
    //   Root (R-SEL, LowerPriority abort)
    //     +-- AlertBranch (SEQ)
    //     |     +-- Alert? (CND, watches "alert" key)
    //     |     +-- Respond (ACT, instant success)
    //     +-- Idle (ACT, long-running)

    let mut builder = BehaviorTreeBuilder::new("debug_overlay");
    let alert = builder.bool_key("alert", BlackboardKeyDirection::Input, false, Some(false));
    let alert_condition = builder.condition_with_watch_keys("Alert?", "alert", [alert]);
    let respond = builder.action("Respond", "respond");
    let alert_branch = builder.sequence("AlertBranch", [alert_condition, respond]);
    let idle = builder.action("Idle", "idle");
    let root = builder.reactive_selector("Root", AbortPolicy::LowerPriority, [alert_branch, idle]);
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition(
            "alert",
            ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(alert).unwrap_or(false)),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "respond",
            ActionHandler::instant(|_ctx| {
                info!("Responding to alert!");
                BehaviorStatus::Success
            }),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "idle",
            ActionHandler::stateful(
                |_ctx| {
                    info!("Idle started");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |_ctx| info!("Idle ABORTED!"),
            ),
        );

    let entity = app
        .world_mut()
        .spawn((
            Name::new("Debug Agent"),
            BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                emit_lifecycle_messages: true,
                restart_on_completion: true,
                trace_capacity: 128,
                ..Default::default()
            }),
            BehaviorTreeDebugRender::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Sprite::from_color(Color::srgb(0.24, 0.63, 0.92), Vec2::new(64.0, 64.0)),
        ))
        .id();

    app.insert_resource(DebugState {
        entity,
        alert_key: alert,
        toggle_timer: Timer::from_seconds(1.2, TimerMode::Repeating),
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            auto_toggle_alert,
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
        "Full debug visualization showcase.\n\
         The tree overlay shows: structure, active path, node status, blackboard, trace, metrics.\n\
         Toggle 'alert' in the pane to trigger reactive abort (watch Idle get aborted).\n\
         Enable 'auto_toggle' to automatically flip the alert flag on an interval.\n\
         The agent sprite color reflects tree status: blue=running, green=success, red=failure.",
    );
}

fn sync_pane(
    pane: Res<DebugPane>,
    mut state: ResMut<DebugState>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut agents: Query<&mut BehaviorTreeAgent>,
    mut blackboards: Query<&mut saddle_ai_behavior_tree::BehaviorTreeBlackboard>,
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
        .toggle_timer
        .set_duration(std::time::Duration::from_secs_f32(
            pane.toggle_interval.max(0.1),
        ));

    if !pane.auto_toggle {
        if let Ok(mut bb) = blackboards.get_mut(state.entity) {
            let _ = bb.set(state.alert_key, pane.alert);
        }
    }
}

fn auto_toggle_alert(
    time: Res<Time>,
    pane: Res<DebugPane>,
    mut state: ResMut<DebugState>,
    mut blackboards: Query<&mut saddle_ai_behavior_tree::BehaviorTreeBlackboard>,
) {
    if !pane.auto_toggle {
        return;
    }
    if state.toggle_timer.tick(time.delta()).just_finished() {
        if let Ok(mut bb) = blackboards.get_mut(state.entity) {
            let current = bb.get_bool(state.alert_key).unwrap_or(false);
            let _ = bb.set(state.alert_key, !current);
        }
    }
}

fn update_monitors(
    state: Res<DebugState>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<DebugPane>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
        pane.abort_count = format!("{}", instance.metrics.abort_count);
        pane.tick_count = format!("{}", instance.metrics.tick_count);
    }
}

fn update_sprite(
    state: Res<DebugState>,
    blackboards: Query<&saddle_ai_behavior_tree::BehaviorTreeBlackboard>,
    instances: Query<&BehaviorTreeInstance>,
    mut sprites: Query<&mut Sprite>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        let alerted = blackboards
            .get(state.entity)
            .ok()
            .and_then(|bb| bb.get_bool(state.alert_key))
            .unwrap_or(false);

        if let Ok(mut sprite) = sprites.get_mut(state.entity) {
            sprite.color = if alerted {
                Color::srgb(0.93, 0.38, 0.22) // orange = alert
            } else {
                match &instance.status {
                    BehaviorTreeRunState::Running => Color::srgb(0.24, 0.63, 0.92),
                    BehaviorTreeRunState::Success => Color::srgb(0.24, 0.84, 0.44),
                    BehaviorTreeRunState::Failure => Color::srgb(0.85, 0.24, 0.28),
                    _ => Color::srgb(0.72, 0.76, 0.84),
                }
            };
        }
    }
}
