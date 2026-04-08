//! Behavior tree — lab (E2E integration)
//!
//! Rich integration scenario demonstrating:
//!
//! - 3D scene with agent (capsule) chasing/patrolling around a target (cube)
//! - Reactive selector with abort: chase vs patrol behavior
//! - Service-driven sensing (distance + visibility checks)
//! - Gizmo debug rendering (ring, active path, target line)
//! - Full tree overlay with blackboard, trace, and metrics
//! - Pane controls for live parameter tuning
//!
//! The agent patrols in a circle. When the target moves close enough and
//! is within the visibility gate, the agent chases it. When the target
//! moves away, the agent resumes patrolling.

#[cfg(feature = "e2e")]
mod e2e;
#[cfg(feature = "e2e")]
mod scenarios;

use std::fmt::Write;

use bevy::gizmos::prelude::AppGizmoBuilder;
use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeDebugGizmos, BehaviorTreeDebugRender, BehaviorTreeHandlers, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems,
    BlackboardKeyDirection, BranchAborted, ConditionHandler, ServiceBinding, ServiceHandler,
    TreeCompleted,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Component)]
pub struct LabAgent;

#[derive(Component)]
struct LabTarget;

#[derive(Component)]
struct LabOverlay;

#[derive(Component)]
struct LabInstructions;

#[derive(Resource, Default)]
pub struct LabStats {
    pub service_ticks: u32,
    pub aborts: u32,
    pub completions: u32,
    pub last_completed_status: Option<BehaviorStatus>,
}

#[derive(Resource, Clone, Pane)]
#[pane(title = "Behavior Tree Lab")]
pub struct BehaviorTreeLabPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    pub time_scale: f32,
    #[pane(slider, min = 1.0, max = 12.0, step = 0.1)]
    pub visibility_radius: f32,
    #[pane(slider, min = -6.0, max = 4.0, step = 0.1)]
    pub visibility_gate_x: f32,
    #[pane(slider, min = 0.5, max = 6.0, step = 0.1)]
    pub chase_speed: f32,
    #[pane(slider, min = 1.0, max = 6.0, step = 0.1)]
    pub patrol_radius: f32,
    #[pane(monitor)]
    pub status: String,
    #[pane(monitor)]
    pub abort_count: String,
}

impl Default for BehaviorTreeLabPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            visibility_radius: 6.0,
            visibility_gate_x: -1.5,
            chase_speed: 2.75,
            patrol_radius: 2.6,
            status: "Idle".into(),
            abort_count: "0".into(),
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        saddle_pane::PanePlugin,
    ));
    app.init_gizmo_group::<BehaviorTreeDebugGizmos>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::BehaviorTreeLabE2EPlugin);
    app.init_resource::<LabStats>();
    app.init_resource::<BehaviorTreeLabPane>();
    app.register_pane::<BehaviorTreeLabPane>();
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_runtime,
            animate_target,
            update_overlay.after(BehaviorTreeSystems::Cleanup),
            update_monitors.after(BehaviorTreeSystems::Cleanup),
            record_runtime_messages.after(BehaviorTreeSystems::Apply),
        ),
    );

    app.run();
}

fn sync_pane_to_runtime(pane: Res<BehaviorTreeLabPane>, mut virtual_time: ResMut<Time<Virtual>>) {
    if pane.is_changed() {
        virtual_time.set_relative_speed(pane.time_scale.max(0.1));
    }
}

fn setup(
    mut commands: Commands,
    mut library: ResMut<BehaviorTreeLibrary>,
    mut handlers: ResMut<BehaviorTreeHandlers>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Lab Camera"),
        Camera3d::default(),
        Transform::from_xyz(-6.0, 7.0, 10.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        Name::new("Lab Light"),
        DirectionalLight {
            illuminance: 14_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Name::new("Lab Ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(24.0, 24.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.14, 0.18, 0.2))),
    ));

    let target = commands
        .spawn((
            Name::new("Lab Target"),
            LabTarget,
            Mesh3d(meshes.add(Cuboid::from_length(0.5))),
            MeshMaterial3d(materials.add(Color::srgb(0.12, 0.72, 0.94))),
            Transform::from_xyz(4.0, 0.25, 0.0),
        ))
        .id();

    let mut builder = BehaviorTreeBuilder::new("behavior_tree_lab");
    let target_entity = builder.entity_key(
        "target_entity",
        BlackboardKeyDirection::Input,
        true,
        Some(target),
    );
    let target_visible = builder.bool_key(
        "target_visible",
        BlackboardKeyDirection::Input,
        false,
        Some(false),
    );
    let distance_to_target = builder.float_key(
        "distance_to_target",
        BlackboardKeyDirection::Output,
        false,
        Some(0.0),
    );

    let visible = builder.condition_with_watch_keys("Visible", "target_visible", [target_visible]);
    let chase = builder.action("Chase", "move_to_target");
    builder.add_tag(chase, "combat");
    let chase_branch = builder.sequence("ChaseBranch", [visible, chase]);
    builder.add_tag(chase_branch, "combat");
    let patrol = builder.action("Patrol", "patrol");
    builder.add_tag(patrol, "patrol");
    let root = builder.reactive_selector(
        "Root",
        saddle_ai_behavior_tree::AbortPolicy::LowerPriority,
        [chase_branch, patrol],
    );
    builder.add_tag(root, "root");
    builder.add_service(
        root,
        ServiceBinding::new("SenseTarget", "sense_target", 0.15).with_watch_keys([
            target_entity,
            target_visible,
            distance_to_target,
        ]),
    );
    builder.set_root(root);

    let definition = builder.build().unwrap();
    let definition_id = library.register(definition).unwrap();
    handlers.register_condition(
        "target_visible",
        ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(target_visible).unwrap_or(false)),
    );
    handlers.register_service(
        "sense_target",
        ServiceHandler::new(move |ctx| {
            let Some(target) = ctx.blackboard.get_entity(target_entity) else {
                return;
            };
            let Some(agent_position) = ctx
                .world
                .get::<GlobalTransform>(ctx.entity)
                .map(GlobalTransform::translation)
            else {
                return;
            };
            let Some(target_position) = ctx
                .world
                .get::<GlobalTransform>(target)
                .map(GlobalTransform::translation)
            else {
                return;
            };

            let to_target = target_position - agent_position;
            let distance = to_target.length();
            let pane = ctx.world.resource::<BehaviorTreeLabPane>();
            let visible =
                distance < pane.visibility_radius && target_position.x > pane.visibility_gate_x;
            ctx.blackboard
                .set(distance_to_target, distance)
                .expect("distance key type should match");
            ctx.blackboard
                .set(target_visible, visible)
                .expect("visibility key type should match");
            ctx.world.resource_mut::<LabStats>().service_ticks += 1;
            if visible {
                ctx.wake_tree("service observed visible target");
            }
        }),
    );
    handlers.register_action(
        "move_to_target",
        ActionHandler::stateful(
            |_ctx| BehaviorStatus::Running,
            move |ctx| {
                let delta_seconds = ctx.world.resource::<Time>().delta_secs();
                let Some(target) = ctx.blackboard.get_entity(target_entity) else {
                    return BehaviorStatus::Failure;
                };
                let Some(target_position) = ctx
                    .world
                    .get::<GlobalTransform>(target)
                    .map(GlobalTransform::translation)
                else {
                    return BehaviorStatus::Failure;
                };
                let Some(current_position) = ctx
                    .world
                    .get::<Transform>(ctx.entity)
                    .map(|t| t.translation)
                else {
                    return BehaviorStatus::Failure;
                };

                let offset = target_position - current_position;
                let distance = offset.length();
                if distance <= 1.2 {
                    return BehaviorStatus::Success;
                }

                let chase_speed = ctx.world.resource::<BehaviorTreeLabPane>().chase_speed;
                let step = offset.normalize_or_zero() * (chase_speed * delta_seconds);
                if let Some(mut transform) = ctx.world.get_mut::<Transform>(ctx.entity) {
                    transform.translation += step;
                }
                BehaviorStatus::Running
            },
            |_ctx| {},
        ),
    );
    handlers.register_action(
        "patrol",
        ActionHandler::stateful(
            |_ctx| BehaviorStatus::Running,
            |ctx| {
                let elapsed = ctx.world.resource::<Time>().elapsed_secs();
                let radius = ctx.world.resource::<BehaviorTreeLabPane>().patrol_radius;
                let desired = Vec3::new(
                    radius * (elapsed * 0.6).cos(),
                    0.55,
                    radius * (elapsed * 0.35).sin(),
                );
                if let Some(mut transform) = ctx.world.get_mut::<Transform>(ctx.entity) {
                    let blend = 0.08;
                    transform.translation = transform.translation.lerp(desired, blend);
                    transform.look_at(Vec3::new(0.0, 0.55, 0.0), Vec3::Y);
                }
                BehaviorStatus::Running
            },
            |_ctx| {},
        ),
    );

    commands.spawn((
        Name::new("Lab Agent"),
        LabAgent,
        BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
            restart_on_completion: true,
            emit_lifecycle_messages: true,
            trace_capacity: 128,
            ..Default::default()
        }),
        BehaviorTreeDebugRender {
            target_entity_key: Some(target_entity),
            ..Default::default()
        },
        Mesh3d(meshes.add(Capsule3d::new(0.35, 0.9))),
        MeshMaterial3d(materials.add(Color::srgb(0.86, 0.46, 0.18))),
        Transform::from_xyz(-2.6, 0.55, 0.0),
    ));

    // Tree overlay (top-left)
    commands.spawn((
        Name::new("Lab Overlay"),
        LabOverlay,
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 0.95, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: px(16.0),
            left: px(16.0),
            max_width: px(520.0),
            ..default()
        },
    ));

    // Instructions (bottom-left)
    commands.spawn((
        Name::new("Lab Instructions"),
        LabInstructions,
        Text::new(
            "3D Lab: agent patrols until the target is visible, then chases.\n\
             Adjust visibility_radius and visibility_gate_x to control sensing.\n\
             The gizmo ring, vertical bars, and target line show debug info.\n\
             Tune chase_speed and patrol_radius for different behaviors.",
        ),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(0.6, 0.65, 0.7, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(16.0),
            left: px(16.0),
            max_width: px(600.0),
            ..default()
        },
    ));
}

fn animate_target(time: Res<Time>, mut targets: Query<&mut Transform, With<LabTarget>>) {
    let Ok(mut transform) = targets.single_mut() else {
        return;
    };
    let elapsed = time.elapsed_secs();
    transform.translation = Vec3::new(
        4.4 * (elapsed * 0.45).cos(),
        0.25,
        2.4 * (elapsed * 0.9).sin(),
    );
}

fn update_overlay(
    stats: Res<LabStats>,
    library: Res<BehaviorTreeLibrary>,
    agents: Query<
        (
            &BehaviorTreeAgent,
            &saddle_ai_behavior_tree::BehaviorTreeInstance,
            &saddle_ai_behavior_tree::BehaviorTreeBlackboard,
        ),
        With<LabAgent>,
    >,
    mut overlays: Query<&mut Text, With<LabOverlay>>,
) {
    let Ok((agent, instance, blackboard)) = agents.single() else {
        return;
    };
    let Ok(mut text) = overlays.single_mut() else {
        return;
    };
    let Some(definition) = library.get(agent.definition) else {
        return;
    };

    // Use the common tree overlay formatter
    text.0 = common::format_tree_overlay(definition, instance, Some(blackboard));

    // Append lab-specific stats
    let mut extra = String::new();
    let _ = writeln!(extra);
    let _ = write!(
        extra,
        "Services: {} | Aborts: {} | Completions: {} ({:?})",
        stats.service_ticks, stats.aborts, stats.completions, stats.last_completed_status
    );
    text.0.push_str(&extra);
}

fn update_monitors(
    stats: Res<LabStats>,
    agents: Query<&BehaviorTreeInstance, With<LabAgent>>,
    mut pane: ResMut<BehaviorTreeLabPane>,
) {
    if let Ok(instance) = agents.single() {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
        pane.abort_count = format!("{}", stats.aborts);
    }
}

fn record_runtime_messages(
    mut stats: ResMut<LabStats>,
    mut aborted: MessageReader<BranchAborted>,
    mut completed: MessageReader<TreeCompleted>,
) {
    for _ in aborted.read() {
        stats.aborts += 1;
    }
    for message in completed.read() {
        stats.completions += 1;
        stats.last_completed_status = Some(message.status);
    }
}
