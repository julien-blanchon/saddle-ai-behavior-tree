# Saddle AI Behavior Tree

Reusable behavior tree runtime for Bevy.

The crate is intentionally generic. It can drive enemy AI, companions, civilians, wildlife, workers, scripted interactions, and low-frequency world agents without importing any project-specific state or gameplay vocabulary.

For apps that keep trees active for the entire app lifetime, prefer `BehaviorTreePlugin::always_on(Update)`. Use `BehaviorTreePlugin::new(...)` when tree activation should be tied to explicit schedules such as `OnEnter` / `OnExit`.

## Quick Start

```toml
[dependencies]
saddle-ai-behavior-tree = { git = "https://github.com/julien-blanchon/saddle-ai-behavior-tree" }
```

```rust
use bevy::prelude::*;
use saddle_ai_behavior_tree::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BehaviorTreePlugin::always_on(Update))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut library: ResMut<BehaviorTreeLibrary>,
    mut handlers: ResMut<BehaviorTreeHandlers>,
) {
    let mut builder = BehaviorTreeBuilder::new("guard");
    let target_visible =
        builder.bool_key("target_visible", BlackboardKeyDirection::Input, false, Some(false));
    let can_attack =
        builder.condition_with_watch_keys("CanAttack", "can_attack", [target_visible]);
    let attack = builder.action("Attack", "attack");
    let patrol = builder.action("Patrol", "patrol");
    let attack_branch = builder.sequence("AttackBranch", [can_attack, attack]);
    let root =
        builder.reactive_selector("Root", AbortPolicy::LowerPriority, [attack_branch, patrol]);
    builder.set_root(root);

    let definition_id = library.register(builder.build().unwrap()).unwrap();
    handlers.register_condition(
        "can_attack",
        ConditionHandler::new(move |ctx| {
            ctx.blackboard.get_bool(target_visible).unwrap_or(false)
        }),
    );
    handlers.register_action(
        "attack",
        ActionHandler::instant(|_ctx| BehaviorStatus::Success),
    );
    handlers.register_action(
        "patrol",
        ActionHandler::stateful(
            |_ctx| BehaviorStatus::Running,
            |_ctx| BehaviorStatus::Running,
            |_ctx| {},
        ),
    );

    commands.spawn((
        Name::new("Guard"),
        BehaviorTreeAgent::new(definition_id).with_config(BehaviorTreeConfig {
            restart_on_completion: true,
            ..Default::default()
        }),
    ));
}
```

## Public API

- Plugin: `BehaviorTreePlugin`
- System sets: `BehaviorTreeSystems::{Prepare, Evaluate, Apply, Cleanup, DebugRender}`
- Components: `BehaviorTreeAgent`, `BehaviorTreeInstance`, `BehaviorTreeBlackboard`, `BehaviorTreeDebugRender`
- Resources: `BehaviorTreeLibrary`, `BehaviorTreeHandlers`, `BehaviorTreeDebugFilter`
- Builder / definition types: `BehaviorTreeBuilder`, `BehaviorTreeDefinition`, `NodeDefinition`, `SubtreeRemap`
- Runtime config: `BehaviorTreeConfig`, `TickMode`, `AbortPolicy`, `ParallelPolicy`, `ServiceBinding`
- Messages: `TreeCompleted`, `NodeStarted`, `NodeFinished`, `BranchAborted`, `TreeWakeRequested`, `TreeResetRequested`, `ActionResolution`, `BlackboardValueChanged`
- Debug / metrics: `BehaviorTreeDebugGizmos`, `BehaviorTreeTrace`, `BehaviorTreeTraceEntry`, `BehaviorTreeMetrics`

## Semantic Guarantees

- Shared definitions, per-entity state:
  definitions are immutable and reusable; entities only store runtime state, metrics, trace data, and blackboard values.
- Stable IDs:
  blackboard keys and nodes resolve to dense indices at build time, so traces and BRP inspection remain stable.
- Explicit update pipeline:
  `Prepare -> Evaluate -> Apply -> Cleanup -> DebugRender`.
- Deterministic reactive aborts:
  lower-priority interruption follows declaration order and explicit `AbortPolicy`.
- Long-running actions are abort-safe:
  stateful actions receive `on_start`, `on_tick`, and `on_abort`.
- Scoped subtree composition:
  subtree input/output remapping happens at build time through `inline_subtree(...)` plus `SubtreeRemap`.

## Building Trees

`BehaviorTreeBuilder` is data-first and reuses stable handler keys instead of storing opaque closures in the definition.

- Composites:
  `sequence`, `sequence_with_memory`, `reactive_sequence`, `selector`, `selector_with_memory`, `reactive_selector`, `parallel`
- Decorators:
  `inverter`, `repeater`, `timeout`, `cooldown`, `retry`, `force_success`, `force_failure`, `succeeder`, `until_success`, `until_failure`, `limiter`, `guard`, `delay`, `run_once`, `blackboard_condition`
- Leaves:
  `action`, `condition`, `condition_with_watch_keys`
- Blackboard keys:
  `bool_key`, `int_key`, `float_key`, `entity_key`, `vec2_key`, `vec3_key`, `quat_key`, `text_key`
- Reuse:
  `inline_subtree("name", &subtree_definition, [SubtreeRemap::new(...), ...])`

Definitions remain reusable because handlers are registered separately in `BehaviorTreeHandlers` with stable string keys.

## Handlers

Register handlers once per app:

- `register_action("key", ActionHandler::instant(...))`
- `register_action("key", ActionHandler::stateful(on_start, on_tick, on_abort))`
- `register_condition("key", ConditionHandler::new(...))`
- `register_service("key", ServiceHandler::new(...))`

Handler contexts expose:

- the controlled entity
- immutable definition data
- typed blackboard access
- node-local memory
- message writes through `Messages<T>`
- wake requests
- async tickets and later completion via `ActionResolution`

## Services

Services are attached to a branch with `add_service(node, ServiceBinding::new(...))`.

- Services run on an explicit interval instead of every frame.
- They can update blackboard keys or cached node-local memory.
- `watch_keys` and `wake_on_change` let services participate in reactive invalidation without brute-force reticking every tree every frame.

## Scoped Blackboards And Remapping

The crate uses explicit key remapping instead of a runtime scope stack:

- subtree-local keys stay local when inlined
- explicitly remapped keys reuse the parent key ID
- non-remapped subtree keys are copied into the parent definition with a stable prefixed name

This keeps runtime lookup flat and cheap while still making subtree inputs/outputs explicit.

## Abort And Wake Semantics

- `AbortPolicy::SelfOnly`:
  reevaluate the running branch and abort it if its own guard becomes invalid.
- `AbortPolicy::LowerPriority`:
  higher-priority reactive branches can abort lower-priority running siblings.
- `AbortPolicy::Both`:
  enable both behaviors.
- `TreeWakeRequested`:
  explicit external wakeup for manual or dormant trees.
- Blackboard dirty tracking:
  watched-key changes wake only trees that care about those keys.

Reactive semantics are covered by unit tests and by the `reactive_abort` example.

## Debugging

Every instance exposes:

- `active_path`
- `status`
- `last_running_leaf`
- `last_abort_reason`
- `wake_reason`
- per-node runtime state
- per-node and per-tree counters
- a bounded `BehaviorTreeTrace`

Line-based gizmo rendering is opt-in:

```rust
use bevy::gizmos::prelude::AppGizmoBuilder;

app.init_gizmo_group::<BehaviorTreeDebugGizmos>();
```

Attach `BehaviorTreeDebugRender` to an agent to draw active-path rings and optional target links. `BehaviorTreeDebugFilter` can narrow rendering by entity, definition, or active-path tag.

## Examples

| Example | Description | Run |
| --- | --- | --- |
| `basic` | Minimal sequence with typed blackboard input and a condition/action pair | `cargo run -p saddle-ai-behavior-tree-example-basic` |
| `reactive_abort` | Higher-priority branch interrupts a lower-priority running patrol | `cargo run -p saddle-ai-behavior-tree-example-reactive-abort` |
| `subtree_scope` | Reusable subtree with explicit input/output remapping | `cargo run -p saddle-ai-behavior-tree-example-subtree-scope` |
| `services` | Interval-driven service updates and logging | `cargo run -p saddle-ai-behavior-tree-example-services` |
| `async_action` | Long-running action completed later through `ActionResolution` | `cargo run -p saddle-ai-behavior-tree-example-async-action` |
| `debug_overlay` | Windowed showcase with UI text plus opt-in debug gizmos | `cargo run -p saddle-ai-behavior-tree-example-debug-overlay` |
| `hot_swap` | Runtime definition replacement with explicit tree reset | `cargo run -p saddle-ai-behavior-tree-example-hot-swap` |
| `stress_test` | Large-agent interval-tick smoke test | `cargo run -p saddle-ai-behavior-tree-example-stress-test --release` |

## More Docs

- [`docs/architecture.md`](docs/architecture.md)
- [`docs/configuration.md`](docs/configuration.md)
