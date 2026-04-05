//! Behavior tree — stress test example
//!
//! Spawns a configurable number of agents (default 2048) each running a
//! simple behavior tree on interval ticks. Displays real-time performance
//! metrics: total ticks, agents count, and frame time.
//!
//! Use this to benchmark the behavior tree runtime. For best results,
//! build with `--release`.

use std::fmt::Write;

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeConfig, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeSystems, ConditionHandler, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

const DEFAULT_AGENT_COUNT: usize = 2_048;

#[derive(Component)]
struct StressOverlay;

#[derive(Resource, Clone, Pane)]
#[pane(title = "Stress Test")]
struct StressPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(monitor)]
    agents: String,
    #[pane(monitor)]
    total_ticks: String,
    #[pane(monitor)]
    frame_time_ms: String,
}

impl Default for StressPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            agents: "0".into(),
            total_ticks: "0".into(),
            frame_time_ms: "0.0".into(),
        }
    }
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / stress_test".into(),
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
    app.register_pane::<StressPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

    let (definition, _) = common::basic_definition();
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    common::register_condition(&mut app, "ready", ConditionHandler::new(|_ctx| true));
    common::register_action(
        &mut app,
        "act",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );

    for _ in 0..DEFAULT_AGENT_COUNT {
        app.world_mut()
            .spawn(
                BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                    tick_mode: TickMode::Interval {
                        seconds: 0.016,
                        phase_offset: 0.0,
                    },
                    restart_on_completion: true,
                    ..Default::default()
                }),
            );
    }

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            update_stress_overlay.after(BehaviorTreeSystems::Cleanup),
            update_monitors.after(BehaviorTreeSystems::Cleanup),
        ),
    );

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Backdrop"),
        Sprite::from_color(Color::srgb(0.07, 0.09, 0.13), Vec2::new(1600.0, 900.0)),
        Transform::from_xyz(0.0, 0.0, -30.0),
    ));

    commands.spawn((
        Name::new("Stress Overlay"),
        StressOverlay,
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 0.95, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: px(12.0),
            left: px(12.0),
            ..default()
        },
    ));

    common::spawn_instructions(
        &mut commands,
        "Stress test: 2048 agents each ticking a simple behavior tree.\n\
         Watch the total tick count and frame time.\n\
         Build with --release for representative performance numbers.\n\
         Adjust time_scale to speed up or slow down the simulation.",
    );
}

fn sync_pane(pane: Res<StressPane>, mut virtual_time: ResMut<Time<Virtual>>) {
    if pane.is_changed() {
        virtual_time.set_relative_speed(pane.time_scale.max(0.1));
    }
}

fn update_stress_overlay(
    time: Res<Time>,
    instances: Query<&BehaviorTreeInstance>,
    mut overlays: Query<&mut Text, With<StressOverlay>>,
) {
    let Ok(mut text) = overlays.single_mut() else {
        return;
    };

    let agent_count = instances.iter().count();
    let total_ticks: u64 = instances
        .iter()
        .map(|instance| instance.metrics.tick_count)
        .sum();
    let frame_ms = time.delta_secs() * 1000.0;

    let mut out = String::with_capacity(256);
    let _ = writeln!(out, "STRESS TEST");
    let _ = writeln!(out, "Agents:      {agent_count}");
    let _ = writeln!(out, "Total ticks: {total_ticks}");
    let _ = writeln!(out, "Frame time:  {frame_ms:.1}ms");
    let _ = writeln!(
        out,
        "FPS:         {:.0}",
        1.0 / time.delta_secs().max(0.001)
    );
    text.0 = out;
}

fn update_monitors(
    time: Res<Time>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<StressPane>,
) {
    let agent_count = instances.iter().count();
    let total_ticks: u64 = instances
        .iter()
        .map(|instance| instance.metrics.tick_count)
        .sum();
    let frame_ms = time.delta_secs() * 1000.0;

    pane.agents = format!("{agent_count}");
    pane.total_ticks = format!("{total_ticks}");
    pane.frame_time_ms = format!("{frame_ms:.1}");
}
