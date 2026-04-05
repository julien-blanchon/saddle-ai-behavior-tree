//! Behavior tree — services example
//!
//! Demonstrates interval-driven service updates attached to nodes.
//!
//! A "Sense" service runs on a configurable interval while the tree is active.
//! Services are used for periodic sensing, blackboard updates, or other
//! background work that should happen independently of the tree's tick rate.
//!
//! The tree repeats an "Idle" action indefinitely while the service ticks
//! in the background. Adjust the service interval and observe the pulse count.

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeHandlers, BehaviorTreeInstance, BehaviorTreeLibrary, BehaviorTreePlugin,
    BehaviorTreeRunState, BehaviorTreeSystems, ServiceBinding, ServiceHandler, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Default)]
struct Pulse(u32);

#[derive(Resource, Clone, Pane)]
#[pane(title = "Services")]
struct ServicesPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    service_pulses: String,
    #[pane(monitor)]
    tick_count: String,
}

impl Default for ServicesPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            status: "Idle".into(),
            service_pulses: "0".into(),
            tick_count: "0".into(),
        }
    }
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / services".into(),
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
    app.register_pane::<ServicesPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.init_resource::<Pulse>();

    // Build tree: Repeater(unlimited) -> Idle, with a "Sense" service on root
    let mut builder = BehaviorTreeBuilder::new("services");
    let idle = builder.action("Idle", "idle");
    let root = builder.repeater("Root", None, idle);
    builder.add_service(root, ServiceBinding::new("Sense", "sense", 0.1));
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    app.world_mut().spawn((
        Name::new("ServiceAgent"),
        BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            restart_on_completion: true,
            trace_capacity: 64,
            ..Default::default()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::from_color(Color::srgb(0.24, 0.63, 0.92), Vec2::new(64.0, 64.0)),
    ));

    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "idle",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_service(
            "sense",
            ServiceHandler::new(|ctx| {
                let mut pulse = ctx.world.resource_mut::<Pulse>();
                pulse.0 += 1;
                info!("service tick {}", pulse.0);
            }),
        );

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            update_monitors,
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
        "Services run on their own interval, independent of the tree tick.\n\
         The 'Sense' service increments a pulse counter every 0.1s.\n\
         The tree repeats an Idle action indefinitely.\n\
         Adjust time_scale to see how services scale with time.",
    );
}

fn sync_pane(
    pane: Res<ServicesPane>,
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

fn update_monitors(
    pulse: Res<Pulse>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<ServicesPane>,
) {
    for instance in &instances {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
        pane.tick_count = format!("{}", instance.metrics.tick_count);
    }
    pane.service_pulses = format!("{}", pulse.0);
}
