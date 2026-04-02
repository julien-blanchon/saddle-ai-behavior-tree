use bevy::prelude::*;

use crate::{
    BehaviorTreePlugin,
    blackboard::BlackboardKeyDirection,
    builder::BehaviorTreeBuilder,
    components::BehaviorTreeAgent,
    debug::BehaviorTreeDebugRender,
    handlers::{ActionHandler, ServiceHandler},
    nodes::BehaviorStatus,
    resources::{BehaviorTreeHandlers, BehaviorTreeLibrary},
};

#[derive(Resource, Default)]
struct ServiceCounter(u32);

#[test]
fn services_run_without_panicking() {
    let mut builder = BehaviorTreeBuilder::new("services");
    let action = builder.action("Work", "work");
    let root = builder.repeater("Root", Some(2), action);
    builder.add_service(
        root,
        crate::nodes::ServiceBinding::new("sense", "sense", 0.0),
    );
    builder.set_root(root);
    let definition = builder.build().unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.init_resource::<ServiceCounter>();
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    {
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_action(
            "work",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
        handlers.register_service(
            "sense",
            ServiceHandler::new(|ctx| {
                ctx.world.resource_mut::<ServiceCounter>().0 += 1;
            }),
        );
    }
    app.world_mut().spawn((
        BehaviorTreeAgent::new(definition_id),
        BehaviorTreeDebugRender::default(),
    ));

    app.update();
    app.update();

    assert!(app.world().resource::<ServiceCounter>().0 > 0);
}

#[test]
fn blackboard_change_wakes_tree() {
    let mut builder = BehaviorTreeBuilder::new("wake");
    let alarm = builder.bool_key("alarm", BlackboardKeyDirection::Input, false, Some(false));
    let root = builder.condition_with_watch_keys("Alarm", "alarm", [alarm]);
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
        .register_condition(
            "alarm",
            crate::handlers::ConditionHandler::new(move |ctx| {
                ctx.blackboard.get_bool(alarm).unwrap_or(false)
            }),
        );
    let entity = app
        .world_mut()
        .spawn(BehaviorTreeAgent::new(definition_id))
        .id();

    app.update();
    let before = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap()
        .metrics
        .tick_count;

    app.world_mut()
        .get_mut::<crate::blackboard::BehaviorTreeBlackboard>(entity)
        .unwrap()
        .set(alarm, true)
        .unwrap();
    crate::systems::prepare_agents(app.world_mut());

    let instance = app
        .world()
        .get::<crate::runtime::BehaviorTreeInstance>(entity)
        .unwrap();
    assert!(instance.wake_requested);
    assert_eq!(instance.metrics.tick_count, before);
}
