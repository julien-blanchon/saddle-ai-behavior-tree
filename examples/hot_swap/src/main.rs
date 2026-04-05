//! Behavior tree — hot swap example
//!
//! Demonstrates runtime definition replacement and tree reset.
//!
//! Two tree definitions are registered: "patrol" and "attack". Press SPACE
//! to swap between them. The tree resets and immediately begins executing
//! the new definition. This is useful for AI state transitions where the
//! entire behavior strategy changes (e.g., switching from exploration to combat).

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeHandlers, BehaviorTreeInstance, BehaviorTreeLibrary, BehaviorTreePlugin,
    BehaviorTreeRunState, BehaviorTreeSystems, TickMode, TreeResetRequested,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Clone, Pane)]
#[pane(title = "Hot Swap")]
struct SwapPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    #[pane(monitor)]
    current_tree: String,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    swap_count: String,
}

impl Default for SwapPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            current_tree: "patrol".into(),
            status: "Idle".into(),
            swap_count: "0".into(),
        }
    }
}

#[derive(Resource)]
struct SwapState {
    entity: Entity,
    patrol_id: saddle_ai_behavior_tree::BehaviorTreeDefinitionId,
    attack_id: saddle_ai_behavior_tree::BehaviorTreeDefinitionId,
    is_patrol: bool,
    swap_count: u32,
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / hot_swap".into(),
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
    app.register_pane::<SwapPane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

    // Build two different tree definitions
    let mut patrol_builder = BehaviorTreeBuilder::new("patrol");
    let patrol_root = patrol_builder.action("Patrol", "patrol");
    patrol_builder.set_root(patrol_root);

    let mut attack_builder = BehaviorTreeBuilder::new("attack");
    let attack_root = attack_builder.action("Attack", "attack");
    attack_builder.set_root(attack_root);

    let patrol_def = patrol_builder.build().unwrap();
    let attack_def = attack_builder.build().unwrap();

    let patrol_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(patrol_def)
        .unwrap();
    let attack_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(attack_def)
        .unwrap();

    let entity = app
        .world_mut()
        .spawn((
            Name::new("SwapAgent"),
            BehaviorTreeAgent::new(patrol_id).with_config(BehaviorTreeConfig {
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
            "patrol",
            ActionHandler::stateful(
                |_ctx| {
                    info!("Patrolling...");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |_ctx| info!("Patrol ended"),
            ),
        );
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "attack",
            ActionHandler::stateful(
                |_ctx| {
                    info!("Attacking!");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |_ctx| info!("Attack ended"),
            ),
        );

    app.insert_resource(SwapState {
        entity,
        patrol_id,
        attack_id,
        is_patrol: true,
        swap_count: 0,
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
            swap_on_space,
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
        "Press SPACE to swap between 'patrol' and 'attack' tree definitions.\n\
         The tree resets instantly and begins executing the new definition.\n\
         Watch the tree overlay update to show the new tree structure.\n\
         The sprite color changes: blue = patrol, orange = attack.",
    );
}

fn sync_pane(
    pane: Res<SwapPane>,
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

fn swap_on_space(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SwapState>,
    mut agents: Query<&mut BehaviorTreeAgent>,
    mut resets: MessageWriter<TreeResetRequested>,
) {
    if keys.just_pressed(KeyCode::Space) {
        state.is_patrol = !state.is_patrol;
        state.swap_count += 1;
        let new_id = if state.is_patrol {
            state.patrol_id
        } else {
            state.attack_id
        };
        if let Ok(mut agent) = agents.get_mut(state.entity) {
            agent.definition = new_id;
            resets.write(TreeResetRequested::new(state.entity, "definition swapped"));
            info!(
                "Swapped to '{}' (swap #{})",
                if state.is_patrol { "patrol" } else { "attack" },
                state.swap_count
            );
        }
    }
}

fn update_monitors(
    state: Res<SwapState>,
    instances: Query<&BehaviorTreeInstance>,
    mut pane: ResMut<SwapPane>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
    }
    pane.current_tree = if state.is_patrol {
        "patrol".into()
    } else {
        "attack".into()
    };
    pane.swap_count = format!("{}", state.swap_count);
}

fn update_sprite(state: Res<SwapState>, mut sprites: Query<&mut Sprite>) {
    if let Ok(mut sprite) = sprites.get_mut(state.entity) {
        sprite.color = if state.is_patrol {
            Color::srgb(0.24, 0.63, 0.92) // blue = patrol
        } else {
            Color::srgb(0.93, 0.58, 0.22) // orange = attack
        };
    }
}
