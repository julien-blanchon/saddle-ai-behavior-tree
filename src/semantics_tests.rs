use bevy::prelude::*;

use crate::{
    BehaviorTreePlugin,
    blackboard::BlackboardKeyDirection,
    builder::BehaviorTreeBuilder,
    components::BehaviorTreeAgent,
    handlers::{ActionHandler, ConditionHandler},
    messages::{BranchAborted, TreeWakeRequested},
    nodes::{AbortPolicy, BehaviorStatus, ParallelPolicy},
    resources::{BehaviorTreeHandlers, BehaviorTreeLibrary},
    runtime::BehaviorTreeConfig,
};

#[derive(Resource, Default, Debug, PartialEq)]
struct TestLog(Vec<&'static str>);

#[derive(Resource, Default)]
struct Counter(u32);

fn spawn_app(definition: crate::definition::BehaviorTreeDefinition) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.add_plugins(BehaviorTreePlugin::always_on(Update));
    app.init_resource::<TestLog>();
    app.init_resource::<Counter>();
    let definition_id = app
        .world_mut()
        .resource_mut::<BehaviorTreeLibrary>()
        .register(definition)
        .unwrap();
    let entity = app
        .world_mut()
        .spawn(BehaviorTreeAgent::new(definition_id))
        .id();
    crate::systems::activate_agents(app.world_mut());
    (app, entity)
}

#[test]
fn sequence_with_memory_resumes_at_failed_child() {
    let mut builder = BehaviorTreeBuilder::new("sequence_memory");
    let a = builder.action("A", "step_a");
    let b = builder.action("B", "step_b");
    let c = builder.action("C", "step_c");
    let root = builder.sequence_with_memory("root", [a, b, c]);
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let (mut app, entity) = spawn_app(definition);
    app.world_mut()
        .get_mut::<BehaviorTreeAgent>(entity)
        .unwrap()
        .config
        .restart_on_completion = true;
    {
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_action(
            "step_a",
            ActionHandler::instant(|ctx| {
                ctx.world.resource_mut::<TestLog>().0.push("A");
                BehaviorStatus::Success
            }),
        );
        handlers.register_action(
            "step_b",
            ActionHandler::instant(|ctx| {
                ctx.world.resource_mut::<TestLog>().0.push("B");
                let mut counter = ctx.world.resource_mut::<Counter>();
                counter.0 += 1;
                if counter.0 == 1 {
                    BehaviorStatus::Failure
                } else {
                    BehaviorStatus::Success
                }
            }),
        );
        handlers.register_action(
            "step_c",
            ActionHandler::instant(|ctx| {
                ctx.world.resource_mut::<TestLog>().0.push("C");
                BehaviorStatus::Success
            }),
        );
    }

    app.update();
    assert_eq!(app.world().resource::<TestLog>().0, vec!["A", "B"]);

    app.update();
    assert_eq!(
        app.world().resource::<TestLog>().0,
        vec!["A", "B", "B", "C"]
    );
}

#[test]
fn reactive_selector_aborts_lower_priority_running_child() {
    let mut builder = BehaviorTreeBuilder::new("reactive_abort");
    let visible = builder.bool_key("visible", BlackboardKeyDirection::Input, false, Some(false));
    let can_attack = builder.condition_with_watch_keys("CanAttack", "can_attack", [visible]);
    let attack = builder.action("Attack", "attack");
    let patrol = builder.action("Patrol", "patrol");
    let attack_branch = builder.sequence("AttackBranch", [can_attack, attack]);
    let root =
        builder.reactive_selector("Root", AbortPolicy::LowerPriority, [attack_branch, patrol]);
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let (mut app, entity) = spawn_app(definition);
    app.world_mut()
        .get_mut::<BehaviorTreeAgent>(entity)
        .unwrap()
        .config = BehaviorTreeConfig {
        emit_lifecycle_messages: true,
        ..Default::default()
    };
    {
        let definition = app.world().resource::<BehaviorTreeLibrary>().definitions[0].clone();
        let visible_key = definition.find_blackboard_key("visible").unwrap();
        app.world_mut()
            .get_mut::<crate::blackboard::BehaviorTreeBlackboard>(entity)
            .unwrap()
            .set(visible_key, false)
            .unwrap();
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_condition(
            "can_attack",
            ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(visible_key).unwrap_or(false)),
        );
        handlers.register_action(
            "attack",
            ActionHandler::instant(|ctx| {
                ctx.world.resource_mut::<TestLog>().0.push("attack");
                BehaviorStatus::Success
            }),
        );
        handlers.register_action(
            "patrol",
            ActionHandler::stateful(
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("patrol_start");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("patrol_abort");
                },
            ),
        );
    }

    app.update();
    assert_eq!(app.world().resource::<TestLog>().0, vec!["patrol_start"]);

    {
        let visible_key = app.world().resource::<BehaviorTreeLibrary>().definitions[0]
            .find_blackboard_key("visible")
            .unwrap();
        app.world_mut()
            .get_mut::<crate::blackboard::BehaviorTreeBlackboard>(entity)
            .unwrap()
            .set(visible_key, true)
            .unwrap();
        app.world_mut()
            .resource_mut::<Messages<TreeWakeRequested>>()
            .write(TreeWakeRequested::new(entity, "target visible"));
    }

    app.update();
    assert!(
        app.world()
            .resource::<TestLog>()
            .0
            .contains(&"patrol_abort")
    );
    assert!(app.world().resource::<TestLog>().0.contains(&"attack"));
    let aborted = app.world().resource::<Messages<BranchAborted>>().len();
    assert!(aborted > 0);
}

#[test]
fn guard_without_self_abort_keeps_running_child_alive() {
    let mut builder = BehaviorTreeBuilder::new("guard_without_self_abort");
    let allowed = builder.bool_key("allowed", BlackboardKeyDirection::Input, false, Some(true));
    let action = builder.action("Action", "action");
    let root = builder.guard(
        "Guard",
        "is_allowed",
        AbortPolicy::LowerPriority,
        [allowed],
        action,
    );
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let (mut app, entity) = spawn_app(definition);
    {
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_condition(
            "is_allowed",
            ConditionHandler::new(move |ctx| ctx.blackboard.get_bool(allowed).unwrap_or(false)),
        );
        handlers.register_action(
            "action",
            ActionHandler::stateful(
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("start");
                    BehaviorStatus::Running
                },
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("tick");
                    BehaviorStatus::Running
                },
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("abort");
                },
            ),
        );
    }

    app.update();
    assert_eq!(app.world().resource::<TestLog>().0, vec!["start"]);

    app.world_mut()
        .get_mut::<crate::blackboard::BehaviorTreeBlackboard>(entity)
        .unwrap()
        .set(allowed, false)
        .unwrap();
    app.world_mut()
        .resource_mut::<Messages<TreeWakeRequested>>()
        .write(TreeWakeRequested::new(entity, "guard changed"));

    app.update();

    let log = &app.world().resource::<TestLog>().0;
    assert_eq!(log, &vec!["start", "tick"]);
    assert!(!log.contains(&"abort"));
    assert_eq!(
        app.world()
            .get::<crate::runtime::BehaviorTreeInstance>(entity)
            .unwrap()
            .status,
        crate::runtime::BehaviorTreeRunState::Running
    );
}

#[test]
fn parallel_success_threshold_aborts_running_sibling() {
    let mut builder = BehaviorTreeBuilder::new("parallel");
    let win = builder.action("Win", "win");
    let wait = builder.action("Wait", "wait");
    let root = builder.parallel(
        "root",
        ParallelPolicy::any_success_all_failure(),
        [win, wait],
    );
    builder.set_root(root);
    let definition = builder.build().unwrap();
    let (mut app, _) = spawn_app(definition);
    {
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_action(
            "win",
            ActionHandler::instant(|_ctx| BehaviorStatus::Success),
        );
        handlers.register_action(
            "wait",
            ActionHandler::stateful(
                |ctx| {
                    ctx.world.resource_mut::<TestLog>().0.push("wait_start");
                    BehaviorStatus::Running
                },
                |_ctx| BehaviorStatus::Running,
                |ctx| ctx.world.resource_mut::<TestLog>().0.push("wait_abort"),
            ),
        );
    }

    app.update();
    assert_eq!(
        app.world().resource::<TestLog>().0,
        vec!["wait_start", "wait_abort"]
    );
}

#[test]
fn subtree_remap_reuses_parent_blackboard_keys() {
    let mut subtree = BehaviorTreeBuilder::new("job_subtree");
    let ready = subtree.bool_key("ready", BlackboardKeyDirection::Input, true, Some(false));
    let _result = subtree.text_key("result", BlackboardKeyDirection::Output, false, None);
    let do_job = subtree.action("DoJob", "do_job");
    let guard = subtree.blackboard_condition(
        "ReadyGuard",
        ready,
        crate::blackboard::BlackboardCondition::IsTrue,
        AbortPolicy::Both,
        do_job,
    );
    subtree.set_root(guard);
    let subtree_definition = subtree.build().unwrap();

    let mut root = BehaviorTreeBuilder::new("root");
    let parent_ready = root.bool_key("ready", BlackboardKeyDirection::Input, true, Some(false));
    let parent_result = root.text_key("result", BlackboardKeyDirection::Output, false, None);
    let subtree_root = root
        .inline_subtree(
            "worker",
            &subtree_definition,
            [
                crate::builder::SubtreeRemap::new("ready", parent_ready),
                crate::builder::SubtreeRemap::new("result", parent_result),
            ],
        )
        .unwrap();
    root.set_root(subtree_root);

    let definition = root.build().unwrap();
    let (mut app, entity) = spawn_app(definition);
    {
        let mut blackboard = app
            .world_mut()
            .get_mut::<crate::blackboard::BehaviorTreeBlackboard>(entity)
            .unwrap();
        blackboard.set(parent_ready, true).unwrap();
        let mut handlers = app.world_mut().resource_mut::<BehaviorTreeHandlers>();
        handlers.register_action(
            "do_job",
            ActionHandler::instant(move |ctx| {
                ctx.blackboard.set(parent_result, "done").unwrap();
                BehaviorStatus::Success
            }),
        );
    }

    app.update();
    let blackboard = app
        .world()
        .get::<crate::blackboard::BehaviorTreeBlackboard>(entity)
        .unwrap();
    assert_eq!(blackboard.get_text(parent_result), Some("done"));
}
