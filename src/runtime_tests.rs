use bevy::prelude::*;

use crate::{
    BehaviorTreePlugin,
    blackboard::BlackboardKeyDirection,
    builder::BehaviorTreeBuilder,
    components::BehaviorTreeAgent,
    handlers::ActionHandler,
    messages::{NodeFinished, NodeStarted, TreeCompleted, TreeWakeRequested},
    nodes::BehaviorStatus,
    resources::{BehaviorTreeHandlers, BehaviorTreeLibrary},
    runtime::{BehaviorTreeConfig, TickMode},
};

#[test]
fn plugin_initializes_core_resources() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));

    assert!(app.world().contains_resource::<BehaviorTreeLibrary>());
    assert!(app.world().contains_resource::<BehaviorTreeHandlers>());
}

#[test]
fn manual_tick_mode_sleeps_until_woken() {
    let mut builder = BehaviorTreeBuilder::new("manual");
    let root = builder.action("Root", "root");
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "root",
            ActionHandler::instant(|_ctx| BehaviorStatus::Running),
        );
    let entity = app
        .world_mut()
        .spawn(
            BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                tick_mode: TickMode::Manual,
                ..Default::default()
            }),
        )
        .id();

    app.update();
    let first_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;

    app.update();
    let second_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;
    assert_eq!(first_tick_count, second_tick_count);

    app.world_mut()
        .resource_mut::<Messages<crate::messages::TreeWakeRequested>>()
        .write(crate::messages::TreeWakeRequested::new(
            entity,
            "manual poke",
        ));
    app.update();
    let third_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;
    assert!(third_tick_count > second_tick_count);
}

#[test]
fn completed_tree_stays_dormant_until_woken_when_restart_disabled() {
    let mut builder = BehaviorTreeBuilder::new("restart");
    let root = builder.action("Root", "root");
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "root",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
    let entity = app
        .world_mut()
        .spawn(BehaviorTreeAgent::new(definition_id))
        .id();

    app.update();
    let first_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;

    app.update();
    let second_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;
    assert_eq!(first_tick_count, second_tick_count);

    app.world_mut()
        .resource_mut::<Messages<TreeWakeRequested>>()
        .write(TreeWakeRequested::new(entity, "restart requested"));
    app.update();
    let third_tick_count = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;
    assert!(third_tick_count > second_tick_count);
}

#[test]
fn interval_tick_mode_respects_initial_phase_offset() {
    let mut builder = BehaviorTreeBuilder::new("interval");
    let root = builder.action("Root", "root");
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "root",
            ActionHandler::instant(|_ctx| BehaviorStatus::Running),
        );
    let entity = app
        .world_mut()
        .spawn(
            BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
                tick_mode: TickMode::Interval {
                    seconds: 1.0,
                    phase_offset: 0.35,
                },
                ..Default::default()
            }),
        )
        .id();

    crate::systems::activate_agents(app.world_mut());

    let instance = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap();
    assert!(!instance.wake_requested);
    assert_eq!(instance.next_tick_at, 0.35);
}

#[test]
fn lifecycle_messages_stay_silent_by_default() {
    let mut builder = BehaviorTreeBuilder::new("messages_default");
    let root = builder.action("Root", "root");
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "root",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
    app.world_mut().spawn(BehaviorTreeAgent::new(definition_id));

    app.update();

    assert!(app.world().resource::<Messages<NodeStarted>>().is_empty());
    assert!(app.world().resource::<Messages<NodeFinished>>().is_empty());
    assert!(app.world().resource::<Messages<TreeCompleted>>().is_empty());
}

#[test]
fn lifecycle_messages_are_emitted_when_enabled() {
    let mut builder = BehaviorTreeBuilder::new("messages");
    let ready = builder.bool_key("ready", BlackboardKeyDirection::Input, false, Some(true));
    let condition = builder.condition_with_watch_keys("Ready", "ready", [ready]);
    let action = builder.action("Action", "act");
    let root = builder.sequence("Root", [condition, action]);
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_condition("ready", crate::handlers::ConditionHandler::new(|_ctx| true));
    app.world_mut()
        .resource_mut::<BehaviorTreeHandlers>()
        .register_action(
            "act",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
    app.world_mut().spawn(
        BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
            emit_lifecycle_messages: true,
            ..Default::default()
        }),
    );

    app.update();

    assert!(!app.world().resource::<Messages<NodeStarted>>().is_empty());
    assert!(!app.world().resource::<Messages<NodeFinished>>().is_empty());
    assert!(!app.world().resource::<Messages<TreeCompleted>>().is_empty());
}
