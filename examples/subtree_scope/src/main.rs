//! Behavior tree — subtree scope example
//!
//! Demonstrates reusable subtree inlining with explicit input/output key
//! remapping via `BehaviorTreeBuilder::inline_subtree`.
//!
//! A "worker" template tree has its own `ready` and `result` keys. The parent
//! tree maps those to its own keys and embeds the worker as a subtree. Toggle
//! `ready` in the pane to gate the subtree execution and observe the result
//! key being written by the subtree action.

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    AbortPolicy, ActionHandler, BehaviorStatus, BehaviorTreeAgent, BehaviorTreeBlackboard,
    BehaviorTreeBuilder, BehaviorTreeConfig, BehaviorTreeHandlers, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems,
    BlackboardCondition, BlackboardKeyDirection, SubtreeRemap, TickMode,
};
use saddle_ai_behavior_tree_example_common as common;
use saddle_pane::prelude::*;

#[derive(Resource, Clone, Pane)]
#[pane(title = "Subtree Scope")]
struct SubtreePane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    time_scale: f32,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    interval_seconds: f32,
    pub ready: bool,
    #[pane(monitor)]
    status: String,
    #[pane(monitor)]
    result: String,
}

impl Default for SubtreePane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            interval_seconds: 0.2,
            ready: true,
            status: "Idle".into(),
            result: "(none)".into(),
        }
    }
}

#[derive(Resource)]
struct SubtreeState {
    entity: Entity,
    ready_key: saddle_ai_behavior_tree::BlackboardKeyId,
    result_key: saddle_ai_behavior_tree::BlackboardKeyId,
}

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree / subtree_scope".into(),
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
    app.register_pane::<SubtreePane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

    // Build the worker template subtree
    let mut template = BehaviorTreeBuilder::new("worker");
    let ready = template.bool_key("ready", BlackboardKeyDirection::Input, true, Some(false));
    let _result = template.text_key("result", BlackboardKeyDirection::Output, false, None);
    let work = template.action("Work", "work");
    let root = template.blackboard_condition(
        "Ready",
        ready,
        BlackboardCondition::IsTrue,
        AbortPolicy::Both,
        work,
    );
    template.set_root(root);
    let template = template.build().unwrap();

    // Build the parent tree that inlines the worker subtree
    let mut builder = BehaviorTreeBuilder::new("subtree_scope");
    let parent_ready = builder.bool_key("ready", BlackboardKeyDirection::Input, true, Some(true));
    let parent_result = builder.text_key("result", BlackboardKeyDirection::Output, false, None);
    let subtree = builder
        .inline_subtree(
            "job_loop",
            &template,
            [
                SubtreeRemap::new("ready", parent_ready),
                SubtreeRemap::new("result", parent_result),
            ],
        )
        .unwrap();
    builder.set_root(subtree);
    let definition = builder.build().unwrap();

    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();

    let entity = app
        .world_mut()
        .spawn((
            Name::new("SubtreeAgent"),
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
            "work",
            ActionHandler::instant(move |ctx| {
                ctx.blackboard
                    .set(parent_result, "subtree completed")
                    .unwrap();
                info!("Work done! Result written to blackboard.");
                BehaviorStatus::Success
            }),
        );

    app.insert_resource(SubtreeState {
        entity,
        ready_key: parent_ready,
        result_key: parent_result,
    });

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane,
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
        "This example inlines a reusable 'worker' subtree with key remapping.\n\
         Toggle 'ready' in the pane to gate the subtree execution.\n\
         Watch the 'result' monitor update when the subtree completes.\n\
         The blackboard section shows how keys are mapped between parent and subtree.",
    );
}

fn sync_pane(
    pane: Res<SubtreePane>,
    state: Res<SubtreeState>,
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

    if let Ok(mut bb) = blackboards.get_mut(state.entity) {
        let _ = bb.set(state.ready_key, pane.ready);
    }
}

fn update_monitors(
    state: Res<SubtreeState>,
    instances: Query<&BehaviorTreeInstance>,
    blackboards: Query<&BehaviorTreeBlackboard>,
    mut pane: ResMut<SubtreePane>,
) {
    if let Ok(instance) = instances.get(state.entity) {
        pane.status = match &instance.status {
            BehaviorTreeRunState::Running => "Running".into(),
            BehaviorTreeRunState::Success => "Success".into(),
            BehaviorTreeRunState::Failure => "Failure".into(),
            _ => "Idle".into(),
        };
    }
    if let Ok(bb) = blackboards.get(state.entity) {
        pane.result = bb.get_text(state.result_key).unwrap_or("(none)").to_owned();
    }
}

fn update_sprite(
    state: Res<SubtreeState>,
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
