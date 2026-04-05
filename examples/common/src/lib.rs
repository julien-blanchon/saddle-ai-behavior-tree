use std::fmt::Write;

use bevy::prelude::*;
use saddle_ai_behavior_tree::{
    ActionHandler, BehaviorTreeAgent, BehaviorTreeBlackboard, BehaviorTreeBuilder,
    BehaviorTreeConfig, BehaviorTreeDefinition, BehaviorTreeHandlers, BehaviorTreeInstance,
    BehaviorTreeLibrary, BehaviorTreePlugin, BehaviorTreeRunState, BehaviorTreeSystems,
    BlackboardKeyDirection, BlackboardKeyId, BlackboardValueChanged, BranchAborted,
    ConditionHandler, DecoratorKind, NodeFinished, NodeId, NodeKind, NodeStarted, SelectorKind,
    SequenceKind, ServiceHandler, TickMode, TraceKind, TreeCompleted,
};
use saddle_pane::prelude::*;

// ---------------------------------------------------------------------------
// Pane — live-tweak parameters and runtime monitors
// ---------------------------------------------------------------------------

#[derive(Resource, Clone, Pane)]
#[pane(title = "Behavior Tree Demo")]
pub struct BehaviorTreeExamplePane {
    #[pane(slider, min = 0.1, max = 2.5, step = 0.05)]
    pub time_scale: f32,
    pub manual_tick_mode: bool,
    #[pane(slider, min = 0.05, max = 2.0, step = 0.05)]
    pub interval_seconds: f32,
    pub ready: bool,
    pub alert: bool,
    pub target_visible: bool,
}

impl Default for BehaviorTreeExamplePane {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            manual_tick_mode: false,
            interval_seconds: 0.2,
            ready: true,
            alert: false,
            target_visible: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Pane plugin bundle
// ---------------------------------------------------------------------------

pub fn pane_plugins() -> (
    bevy_flair::FlairPlugin,
    bevy_input_focus::InputDispatchPlugin,
    bevy_ui_widgets::UiWidgetsPlugins,
    bevy_input_focus::tab_navigation::TabNavigationPlugin,
    saddle_pane::PanePlugin,
) {
    (
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        saddle_pane::PanePlugin,
    )
}

// ---------------------------------------------------------------------------
// Standard app setup — 2D scene with BT plugin and pane
// ---------------------------------------------------------------------------

pub fn headless_app() -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.08)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "behavior_tree demo".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(pane_plugins());
    app.register_pane::<BehaviorTreeExamplePane>();
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            sync_pane_to_runtime,
            decorate_agents,
            update_agent_visuals,
            drift_agents,
        ),
    );
    app
}

// ---------------------------------------------------------------------------
// Scene — camera + colored lanes for context
// ---------------------------------------------------------------------------

fn setup_scene(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), Camera2d));
    commands.spawn((
        Name::new("Backdrop"),
        Sprite::from_color(Color::srgb(0.07, 0.09, 0.13), Vec2::new(1600.0, 900.0)),
        Transform::from_xyz(0.0, 0.0, -30.0),
    ));
    commands.spawn((
        Name::new("Combat Lane"),
        Sprite::from_color(
            Color::srgba(0.54, 0.18, 0.14, 0.24),
            Vec2::new(1160.0, 140.0),
        ),
        Transform::from_xyz(0.0, 120.0, -20.0),
    ));
    commands.spawn((
        Name::new("Patrol Lane"),
        Sprite::from_color(
            Color::srgba(0.17, 0.39, 0.61, 0.24),
            Vec2::new(1160.0, 140.0),
        ),
        Transform::from_xyz(0.0, -120.0, -20.0),
    ));
}

// ---------------------------------------------------------------------------
// Tree + agent registration helpers
// ---------------------------------------------------------------------------

pub fn register_tree(
    app: &mut App,
    definition: saddle_ai_behavior_tree::BehaviorTreeDefinition,
    config: BehaviorTreeConfig,
) -> (Entity, saddle_ai_behavior_tree::BehaviorTreeDefinitionId) {
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    let entity = app
        .world_mut()
        .spawn((
            BehaviorTreeAgent::new(definition_id).with_config(config),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    (entity, definition_id)
}

pub fn register_action(app: &mut App, name: &str, handler: ActionHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(name, handler);
}

pub fn register_condition(app: &mut App, name: &str, handler: ConditionHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition(name, handler);
}

pub fn register_service(app: &mut App, name: &str, handler: ServiceHandler) {
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_service(name, handler);
}

pub fn basic_definition() -> (
    saddle_ai_behavior_tree::BehaviorTreeDefinition,
    BlackboardKeyId,
) {
    let mut builder = BehaviorTreeBuilder::new("basic");
    let ready = builder.bool_key("ready", BlackboardKeyDirection::Input, false, Some(true));
    let condition = builder.condition_with_watch_keys("Ready", "ready", [ready]);
    let action = builder.action("Act", "act");
    let root = builder.sequence("Root", [condition, action]);
    builder.set_root(root);
    (builder.build().unwrap(), ready)
}

// ---------------------------------------------------------------------------
// Agent visual decoration
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

fn sync_pane_to_runtime(
    pane: Res<BehaviorTreeExamplePane>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut agents: Query<&mut BehaviorTreeAgent>,
    mut blackboards: Query<&mut BehaviorTreeBlackboard>,
) {
    if !pane.is_changed() {
        return;
    }

    virtual_time.set_relative_speed(pane.time_scale.max(0.1));

    for mut agent in &mut agents {
        agent.config.tick_mode = if pane.manual_tick_mode {
            TickMode::Manual
        } else {
            TickMode::Interval {
                seconds: pane.interval_seconds.max(0.01),
                phase_offset: 0.0,
            }
        };
    }

    for mut blackboard in &mut blackboards {
        for (key_name, value) in [
            ("ready", pane.ready),
            ("alert", pane.alert),
            ("target_visible", pane.target_visible),
        ] {
            if let Some(key) = blackboard.schema.find_key(key_name) {
                let _ = blackboard.set(key, value);
            }
        }
    }
}

fn update_agent_visuals(
    instances: Query<(
        Entity,
        &BehaviorTreeInstance,
        Option<&BehaviorTreeBlackboard>,
    )>,
    mut sprites: Query<&mut Sprite, With<BehaviorTreeAgent>>,
) {
    for (entity, instance, blackboard) in &instances {
        let Ok(mut sprite) = sprites.get_mut(entity) else {
            continue;
        };

        let alerted = blackboard
            .and_then(|board| {
                board
                    .schema
                    .find_key("alert")
                    .and_then(|key| board.get_bool(key))
            })
            .unwrap_or(false);

        sprite.color = match (instance.status.clone(), alerted) {
            (_, true) => Color::srgb(0.93, 0.38, 0.22),
            (BehaviorTreeRunState::Running, false) => Color::srgb(0.24, 0.63, 0.92),
            (BehaviorTreeRunState::Success, false) => Color::srgb(0.24, 0.84, 0.44),
            (BehaviorTreeRunState::Failure, false) => Color::srgb(0.85, 0.24, 0.28),
            _ => Color::srgb(0.72, 0.76, 0.84),
        };
    }
}

fn drift_agents(time: Res<Time>, mut agents: Query<&mut Transform, With<BehaviorTreeAgent>>) {
    let now = time.elapsed_secs();
    for (index, mut transform) in agents.iter_mut().enumerate() {
        let lane = if index % 2 == 0 { 120.0 } else { -120.0 };
        transform.translation.x = (now * 1.2 + index as f32).sin() * 260.0;
        transform.translation.y = lane + (now * 2.4 + index as f32).cos() * 16.0;
    }
}

// ---------------------------------------------------------------------------
// Lifecycle message logging
// ---------------------------------------------------------------------------

pub fn add_logging_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            log_started,
            log_finished,
            log_completed,
            log_aborts,
            log_blackboard_changes,
        )
            .after(BehaviorTreeSystems::Apply),
    );
}

fn log_started(mut reader: MessageReader<NodeStarted>) {
    for message in reader.read() {
        info!("start: {}", message.path);
    }
}

fn log_finished(mut reader: MessageReader<NodeFinished>) {
    for message in reader.read() {
        info!("finish: {} -> {:?}", message.path, message.status);
    }
}

fn log_completed(mut reader: MessageReader<TreeCompleted>) {
    for message in reader.read() {
        info!(
            "tree completed: {:?} on {:?}",
            message.status, message.entity
        );
    }
}

fn log_aborts(mut reader: MessageReader<BranchAborted>) {
    for message in reader.read() {
        info!("abort: {} ({})", message.path, message.reason);
    }
}

fn log_blackboard_changes(mut reader: MessageReader<BlackboardValueChanged>) {
    for message in reader.read() {
        info!("blackboard: {} -> {:?}", message.name, message.new_value);
    }
}

// ===========================================================================
// Tree visualization overlay
// ===========================================================================

/// Marker component for the tree overlay text entity.
#[derive(Component)]
pub struct TreeOverlay;

/// Marker component for the instructions text entity.
#[derive(Component)]
pub struct InstructionsText;

/// Spawns a tree visualization overlay in the top-left corner of the screen.
pub fn spawn_tree_overlay(commands: &mut Commands) {
    commands.spawn((
        Name::new("Tree Overlay"),
        TreeOverlay,
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 0.95, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: px(12.0),
            left: px(12.0),
            max_width: px(520.0),
            ..default()
        },
    ));
}

/// Spawns an instructions text box at the bottom-left of the screen.
pub fn spawn_instructions(commands: &mut Commands, text: &str) {
    commands.spawn((
        Name::new("Instructions"),
        InstructionsText,
        Text::new(text.to_owned()),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(0.6, 0.65, 0.7, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12.0),
            left: px(12.0),
            max_width: px(600.0),
            ..default()
        },
    ));
}

/// System that updates the tree overlay text for the first agent found.
pub fn update_tree_overlay(
    library: Res<BehaviorTreeLibrary>,
    agents: Query<(
        &BehaviorTreeAgent,
        &BehaviorTreeInstance,
        Option<&BehaviorTreeBlackboard>,
    )>,
    mut overlays: Query<&mut Text, With<TreeOverlay>>,
) {
    let Ok(mut text) = overlays.single_mut() else {
        return;
    };
    let Some((agent, instance, blackboard)) = agents.iter().next() else {
        text.0 = "No agent found.".into();
        return;
    };
    let Some(definition) = library.get(agent.definition) else {
        text.0 = "Definition not found.".into();
        return;
    };

    text.0 = format_tree_overlay(definition, instance, blackboard);
}

/// Builds the full tree overlay string.
pub fn format_tree_overlay(
    definition: &BehaviorTreeDefinition,
    instance: &BehaviorTreeInstance,
    blackboard: Option<&BehaviorTreeBlackboard>,
) -> String {
    let mut out = String::with_capacity(1024);

    // Header
    let status_sym = run_state_symbol(&instance.status);
    let status_label = run_state_label(&instance.status);
    let _ = writeln!(
        out,
        "Behavior Tree: \"{}\"    {} {}",
        definition.name, status_sym, status_label
    );

    // Active path breadcrumb
    let breadcrumb: Vec<&str> = instance
        .active_path
        .iter()
        .filter_map(|id| definition.node(*id).map(|n| n.name.as_str()))
        .collect();
    if breadcrumb.is_empty() {
        let _ = writeln!(out, "Path: (idle)");
    } else {
        let _ = writeln!(out, "Path: {}", breadcrumb.join(" > "));
    }
    let _ = writeln!(out);

    // Tree structure
    format_node_recursive(definition, instance, definition.root, 0, &mut out);
    let _ = writeln!(out);

    // Blackboard
    if let Some(bb) = blackboard {
        let _ = writeln!(out, "Blackboard:");
        if bb.schema.keys.is_empty() {
            let _ = writeln!(out, "  (empty)");
        } else {
            for key_def in &bb.schema.keys {
                let value_str = bb
                    .value(key_def.id)
                    .map(format_blackboard_value)
                    .unwrap_or_else(|| "(unset)".into());
                let _ = writeln!(out, "  {} = {}", key_def.name, value_str);
            }
        }
        let _ = writeln!(out);
    }

    // Trace (last 8 entries)
    if !instance.trace.entries.is_empty() {
        let _ = writeln!(out, "Trace (recent):");
        let start = instance.trace.entries.len().saturating_sub(8);
        for entry in &instance.trace.entries[start..] {
            let node_name = definition
                .node(entry.node)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            let kind_str = trace_kind_label(entry.kind);
            let status_str = entry
                .status
                .as_ref()
                .map(|s| format!(" ({s:?})"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  #{:<5} {:<16} {}{}",
                entry.frame, node_name, kind_str, status_str
            );
        }
        let _ = writeln!(out);
    }

    // Metrics footer
    let m = &instance.metrics;
    let _ = write!(
        out,
        "Ticks: {}  |  Last: {}us  |  Aborts: {}",
        m.tick_count, m.last_tick_micros, m.abort_count
    );

    out
}

// ---------------------------------------------------------------------------
// Tree rendering helpers
// ---------------------------------------------------------------------------

fn format_node_recursive(
    definition: &BehaviorTreeDefinition,
    instance: &BehaviorTreeInstance,
    node_id: NodeId,
    depth: usize,
    out: &mut String,
) {
    let Some(node) = definition.node(node_id) else {
        return;
    };
    let indent = "  ".repeat(depth);
    let kind_label = node_kind_label(&node.kind);
    let on_active_path = instance.active_path.contains(&node_id);

    let state = instance
        .node_states
        .get(node_id.0 as usize)
        .map(|s| &s.status)
        .unwrap_or(&BehaviorTreeRunState::Idle);

    let marker = if on_active_path { ">" } else { " " };
    let symbol = run_state_symbol(state);
    let label = run_state_label(state);

    let exec_count = instance
        .node_states
        .get(node_id.0 as usize)
        .map(|s| s.execution_count)
        .unwrap_or(0);
    let count_str = if exec_count > 0 {
        format!(" x{exec_count}")
    } else {
        String::new()
    };

    let _ = writeln!(
        out,
        "{marker} {indent}[{kind_label}] {name:<24} {symbol} {label}{count_str}",
        name = node.name
    );

    for child in &node.children {
        format_node_recursive(definition, instance, *child, depth + 1, out);
    }
}

fn node_kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Sequence(SequenceKind::Sequence) => "SEQ",
        NodeKind::Sequence(SequenceKind::SequenceWithMemory) => "SEQ*",
        NodeKind::Sequence(SequenceKind::ReactiveSequence) => "R-SEQ",
        NodeKind::Selector(SelectorKind::Selector) => "SEL",
        NodeKind::Selector(SelectorKind::SelectorWithMemory) => "SEL*",
        NodeKind::Selector(SelectorKind::ReactiveSelector { .. }) => "R-SEL",
        NodeKind::Parallel(_) => "PAR",
        NodeKind::Decorator(dec) => decorator_label(dec),
        NodeKind::Action(_) => "ACT",
        NodeKind::Condition { .. } => "CND",
    }
}

fn decorator_label(dec: &DecoratorKind) -> &'static str {
    match dec {
        DecoratorKind::Inverter => "INV",
        DecoratorKind::Repeater { .. } => "RPT",
        DecoratorKind::Timeout { .. } => "TMO",
        DecoratorKind::Cooldown { .. } => "CDN",
        DecoratorKind::Retry { .. } => "RTY",
        DecoratorKind::ForceSuccess => "F-OK",
        DecoratorKind::ForceFailure => "F-FL",
        DecoratorKind::Succeeder => "SUCC",
        DecoratorKind::UntilSuccess { .. } => "U-OK",
        DecoratorKind::UntilFailure { .. } => "U-FL",
        DecoratorKind::Limiter { .. } => "LIM",
        DecoratorKind::Guard { .. } => "GRD",
        DecoratorKind::Delay { .. } => "DLY",
        DecoratorKind::RunOnce { .. } => "ONCE",
        DecoratorKind::BlackboardCondition { .. } => "BB?",
    }
}

fn run_state_symbol(state: &BehaviorTreeRunState) -> &'static str {
    match state {
        BehaviorTreeRunState::Running => "~",
        BehaviorTreeRunState::Success => "+",
        BehaviorTreeRunState::Failure => "x",
        BehaviorTreeRunState::Idle => ".",
        BehaviorTreeRunState::Deactivated => "o",
    }
}

fn run_state_label(state: &BehaviorTreeRunState) -> &'static str {
    match state {
        BehaviorTreeRunState::Running => "RUNNING",
        BehaviorTreeRunState::Success => "SUCCESS",
        BehaviorTreeRunState::Failure => "FAILURE",
        BehaviorTreeRunState::Idle => "IDLE",
        BehaviorTreeRunState::Deactivated => "OFF",
    }
}

fn trace_kind_label(kind: TraceKind) -> &'static str {
    match kind {
        TraceKind::Started => "STARTED",
        TraceKind::Finished => "FINISHED",
        TraceKind::Aborted => "ABORTED",
        TraceKind::Service => "SERVICE",
        TraceKind::Wake => "WAKE",
        TraceKind::BlackboardChanged => "BB-CHANGED",
    }
}

fn format_blackboard_value(value: &saddle_ai_behavior_tree::BlackboardValue) -> String {
    use saddle_ai_behavior_tree::BlackboardValue;
    match value {
        BlackboardValue::Bool(v) => format!("{v}"),
        BlackboardValue::Int(v) => format!("{v}"),
        BlackboardValue::Float(v) => format!("{v:.2}"),
        BlackboardValue::Entity(v) => format!("{v:?}"),
        BlackboardValue::Vec2(v) => format!("({:.1}, {:.1})", v.x, v.y),
        BlackboardValue::Vec3(v) => format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z),
        BlackboardValue::Quat(v) => format!("({:.2}, {:.2}, {:.2}, {:.2})", v.x, v.y, v.z, v.w),
        BlackboardValue::Text(v) => format!("\"{v}\""),
    }
}
