//! Behavior tree — async action example
//!
//! Demonstrates long-running actions with ticket-based external completion.
//!
//! The "AsyncWork" action requests an async ticket on start, then waits for
//! an external system to resolve it. Press SPACE to resolve the pending
//! action and watch the tree complete.
//!
//! This pattern is useful for actions that depend on external systems
//! (network requests, AI planning, animations) that complete asynchronously.

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, ActionResolution, ActionTicket, BehaviorStatus, BehaviorTreeAgent,
    BehaviorTreeBuilder, BehaviorTreeConfig, BehaviorTreeHandlers, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Default)]
struct PendingTicket(Option<ActionTicket>);

#[derive(Resource)]
struct AsyncState {
    entity: Entity,
    resolve_count: u32,
}

#[derive(Resource, Clone, Pane)]
#[pane(title = "Async Action")]
struct AsyncPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    pending: String,
    #[pane(monitor)]
    resolve_count: String,
}

impl Default for AsyncPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            status: "Idle".into(),
            pending: "No".into(),
            resolve_count: "0".into(),
        }
    }
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / async_action".into(),
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
    app.register_pane::<AsyncPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.init_resource::<PendingTicket>();

    let mut builder = BehaviorTreeBuilder::new("async");
    let root = builder.action("AsyncWork", "async_work");
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
            Name::new("AsyncAgent"),
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
        .register_action(
            "async_work",
            ActionHandler::stateful(
                |ctx| {
                    let ticket = ctx.request_async_ticket();
                    ctx.world.resource_mut::<PendingTicket>().0 = Some(ticket);
                    info!("Async action started, ticket issued. Press SPACE to resolve.");
                    BehaviorStatus::Running
                },
                |ctx| {
                    ctx.take_async_resolution()
                        .unwrap_or(BehaviorStatus::Running)
                },
                |_ctx| {
                    info!("Async action aborted.");
                },
            ),
        );

    app.insert_resource(AsyncState {
        entity,
        resolve_count: 0,
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            resolve_on_space,
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
        "Press SPACE to resolve the pending async action.\n\
         The action requests a ticket on start and waits for external resolution.\n\
         Watch the tree status change from RUNNING to SUCCESS when resolved.\n\
         The tree restarts automatically, issuing a new ticket each time.",
    );
}

fn sync_pane(
    pane: Res<AsyncPane>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut agents: Query<&mut BehaviorTreeAgent>,
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
}

fn resolve_on_space(
    keys: Res<ButtonInput<KeyCode>>,
    mut pending: ResMut<PendingTicket>,
    mut state: ResMut<AsyncState>,
    mut writer: MessageWriter<ActionResolution>,
) {
    if keys.just_pressed(KeyCode::Space) {
        if let Some(ticket) = pending.0.take() {
            writer.write(ActionResolution::new(
                state.entity,
                ticket,
                BehaviorStatus::Success,
            ));
            state.resolve_count += 1;
            info!(
                "Async action resolved via SPACE! (count: {})",
                state.resolve_count
            );
        } else {
            info!("No pending ticket to resolve.");
        }
    }
}

fn update_monitors(
    state: Res<AsyncState>,
    pending: Res<PendingTicket>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<AsyncPane>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
    }
    pane.pending = if pending.0.is_some() {
        "Yes (press SPACE)".into()
    } else {
        "No".into()
    };
    pane.resolve_count = format!("{}", state.resolve_count);
}

fn update_sprite(
    state: Res<AsyncState>,
    pending: Res<PendingTicket>,
    instances: Query<&BehaviorTreeInstance>,
    mut sprites: Query<&mut Sprite>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        if let Ok(mut sprite) = sprites.get_mut(state.entity) {
            // Pulse the color when waiting for resolution
            sprite.color = if pending.0.is_some() {
                Color::srgb(0.93, 0.68, 0.22) // amber = waiting
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
