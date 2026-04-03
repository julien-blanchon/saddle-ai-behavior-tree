use bevy::gizmos::prelude::AppGizmoBuilder;
use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBuilder, BehaviorTreeConfig,
    BehaviorTreeDebugGizmos, BehaviorTreeDebugRender, BehaviorTreeHandlers, BehaviorTreeLibrary,
    BehaviorTreePlugin, BehaviorTreeSystems, BlackboardKeyDirection, BlackboardKeyId,
    BranchAborted, ConditionHandler, ServiceBinding, ServiceHandler, TreeCompleted,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct LabAgent;

#[derive(Component)]
struct LabTarget;

#[derive(Component)]
struct OverlayText;

#[derive(Resource, Clone, Copy)]
struct LabKeys {
    target_visible: BlackboardKeyId,
    distance_to_target: BlackboardKeyId,
}

#[derive(Resource, Default)]
struct LabStats {
    service_ticks: u32,
    aborts: u32,
    completions: u32,
    last_completed_status: Option<BehaviorStatus>,
}

#[derive(Resource, Clone, Pane)]
#[pane(title = "Behavior Tree Lab")]
struct BehaviorTreeLabPane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 1.0, max = 12.0, step = 0.1)]
    visibility_radius: f32,
    #[pane(slider, min = -6.0, max = 4.0, step = 0.1)]
    visibility_gate_x: f32,
    #[pane(slider, min = 0.5, max = 6.0, step = 0.1)]
    chase_speed: f32,
    #[pane(slider, min = 1.0, max = 6.0, step = 0.1)]
    patrol_radius: f32,
}

impl Default for BehaviorTreeLabPane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            visibility_radius: 6.0,
            visibility_gate_x: -1.5,
            chase_speed: 2.75,
            patrol_radius: 2.6,
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
    app.init_resource::<LabStats>();
    app.init_resource::<BehaviorTreeLabPane>();
    app.register_pane::<BehaviorTreeLabPane>();
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        (
            sync_pane_to_runtime,
            animate_target,
            sync_overlay.after(BehaviorTreeSystems::Cleanup),
            record_runtime_messages.after(BehaviorTreeSystems::Apply),
        ),
    );

    app.run();
}

fn sync_pane_to_runtime(
    pane: Res<BehaviorTreeLabPane>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
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
    commands.insert_resource(LabKeys {
        target_visible,
        distance_to_target,
    });

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

    commands.spawn((
        Name::new("Lab Overlay"),
        OverlayText,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: px(16.0),
            left: px(16.0),
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

fn sync_overlay(
    stats: Res<LabStats>,
    keys: Res<LabKeys>,
    library: Res<BehaviorTreeLibrary>,
    agents: Query<
        (
            &BehaviorTreeAgent,
            &saddle_ai_behavior_tree::BehaviorTreeInstance,
            &saddle_ai_behavior_tree::BehaviorTreeBlackboard,
        ),
        With<LabAgent>,
    >,
    mut overlays: Query<&mut Text, With<OverlayText>>,
) {
    let Ok((agent, instance, blackboard)) = agents.single() else {
        return;
    };
    let Ok(mut text) = overlays.single_mut() else {
        return;
    };
    let active_nodes = library
        .get(agent.definition)
        .map(|definition| {
            instance
                .active_path
                .iter()
                .filter_map(|node| {
                    definition
                        .node(*node)
                        .map(|definition| definition.name.clone())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    text.0 = format!(
        "active path: {:?}\nstatus: {:?}\nvisible: {:?}\ndistance: {:?}\nservices: {}\naborts: {}\ncompletions: {} ({:?})",
        active_nodes,
        instance.status,
        blackboard.get_bool(keys.target_visible),
        blackboard
            .get_float(keys.distance_to_target)
            .map(|value| format!("{value:.2}")),
        stats.service_ticks,
        stats.aborts,
        stats.completions,
        stats.last_completed_status,
    );
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
