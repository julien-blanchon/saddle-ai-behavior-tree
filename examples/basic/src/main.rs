//! Behavior tree — basic example
//!
//! Demonstrates the complete workflow for a minimal behavior tree:
//!
//! 1. Build a tree definition with `BehaviorTreeBuilder`
//! 2. Register blackboard keys, conditions, and actions
//! 3. Spawn an agent entity with `BehaviorTreeAgent`
//! 4. Observe runtime status via the tree overlay and visual feedback
//!
//! The tree has a single **sequence**: a "Ready" condition gate followed by an
//! "Act" action. Toggle the `ready` flag in the pane to watch the tree succeed
//! or stall.

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBlackboard, BehaviorTreeBuilder,
    BehaviorTreeConfig, BehaviorTreeHandlers, BehaviorTreeInstance, BehaviorTreeLibrary,
    BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems, BlackboardKeyDirection,
    BlackboardKeyId, ConditionHandler, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

// ---------------------------------------------------------------------------
// Pane — live-tweak parameters and runtime monitors
// ---------------------------------------------------------------------------

#[derive(Resource, Clone, Pane)]
#[pane(title = "Behavior Tree — Basic")]
struct BasicPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    pub ready: bool,
    #[pane(monitor)]
    status: String,
}

impl Default for BasicPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            ready: true,
            status: "Idle".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let mut app = App::new();

    // --- Window & rendering ---
    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / basic".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }));

    // --- Pane ---
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        PanePlugin,
    ));
    app.register_pane::<BasicPane>();

    // --- Behavior tree plugin ---
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

    // --- Build the tree definition ---
    //
    // Structure:
    //   Sequence "Root"
    //     +-- Condition "Ready"   (watches blackboard key `ready`)
    //     +-- Action "Act"        (instant -- logs and succeeds)
    //
    let mut builder = BehaviorTreeBuilder::new("basic");

    // Blackboard key: a boolean the pane can toggle
    let ready_key: BlackboardKeyId =
        builder.bool_key("ready", BlackboardKeyDirection::Input, false, Some(true));

    let condition_node = builder.condition_with_watch_keys("Ready", "ready", [ready_key]);
    let action_node = builder.action("Act", "act");
    let root = builder.sequence("Root", [condition_node, action_node]);
    builder.set_root(root);
    let definition = builder.build().unwrap();

    // --- Register definition in the library and spawn an agent ---
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    app.world_mut().spawn((
        Name::new("BasicAgent"),
        BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            restart_on_completion: true,
            trace_capacity: 64,
            ..Default::default()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // --- Register handlers ---
    // The condition reads the blackboard `ready` key.
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition(
            "ready",
            ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(ready_key).unwrap_or(false)),
        );

    // The action simply logs and succeeds instantly.
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "act",
            ActionHandler::instant(|ctx| {
                info!("Entity {:?} performed the basic action!", ctx.entity);
                BehaviorStatus::Success
            }),
        );

    // --- Visual & runtime systems ---
    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane_to_runtime,
            update_pane_monitors,
            decorate_agents,
            update_agent_visuals,
            common::update_tree_overlay.after(BehaviorTreeSystems::Cleanup),
        ),
    );

    app.run();
}

// ---------------------------------------------------------------------------
// Scene — camera + sprite + tree overlay
// ---------------------------------------------------------------------------

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
        "Toggle 'ready' in the pane to gate the sequence.\n\
         When ready=true the tree runs Root > Ready > Act and succeeds.\n\
         When ready=false the condition fails and the sequence fails.\n\
         Adjust time_scale and interval_seconds to control tick rate.",
    );
}

// ---------------------------------------------------------------------------
// Auto-decorate agent entities with a sprite if they don't have one
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Pane -> runtime sync (toggle ready flag, adjust tick speed)
// ---------------------------------------------------------------------------

fn sync_pane_to_runtime(
    pane: Res<BasicPane>,
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

    for mut blackboard in &mut blackboards {
        if let Some(key) = blackboard.schema.find_key("ready") {
            let _ = blackboard.set(key, pane.ready);
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime -> pane monitors
// ---------------------------------------------------------------------------

fn update_pane_monitors(instances: Query<&BehaviorTreeInstance>, mut pane: ResMut<BasicPane>) {
    for instance in &instances {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
    }
}

// ---------------------------------------------------------------------------
// Color the agent sprite based on tree status
// ---------------------------------------------------------------------------

fn update_agent_visuals(
    instances: Query<(Entity, &BehaviorTreeInstance)>,
    mut sprites: Query<&mut Sprite, With<BehaviorTreeAgent>>,
) {
    for (entity, instance) in &instances {
        let Ok(mut sprite) = sprites.get_mut(entity) else {
            continue;
        };
        sprite.color = match &instance.status {
            BehaviorTreeRunState::Running => Color::srgb(0.24, 0.63, 0.92),
            BehaviorTreeRunState::Success => Color::srgb(0.24, 0.84, 0.44),
            BehaviorTreeRunState::Failure => Color::srgb(0.85, 0.24, 0.28),
            _ => Color::srgb(0.72, 0.76, 0.84),
        };
    }
}
