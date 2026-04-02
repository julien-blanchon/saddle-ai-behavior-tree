# Configuration

This document lists the main tuning points exposed by `saddle-ai-behavior-tree` in v0.1.

## Plugin Schedules

Use `BehaviorTreePlugin::always_on(update_schedule)` when trees should stay active for the app lifetime. Use `BehaviorTreePlugin::new(activate, deactivate, update)` when tree activation should be tied to explicit schedules.

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `activate_schedule` | `Interned<dyn ScheduleLabel>` | none | Schedule where agents get runtime state and blackboards initialized or refreshed |
| `deactivate_schedule` | `Interned<dyn ScheduleLabel>` | none | Schedule where runtime state is removed and blackboard dirty state is cleared |
| `update_schedule` | `Interned<dyn ScheduleLabel>` | none | Schedule that runs the full behavior-tree pipeline |

## `BehaviorTreeConfig`

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `tick_mode` | `TickMode` | `EveryFrame` | Controls whether the tree ticks every frame, on an interval, or only when explicitly woken |
| `restart_on_completion` | `bool` | `false` | When `true`, a completed root restarts on its next eligible automatic tick; when `false`, completion sleeps until a wake or reset arrives |
| `preserve_blackboard_on_definition_change` | `bool` | `true` | Reuses matching blackboard values by name and type when an entity swaps to another definition |
| `emit_lifecycle_messages` | `bool` | `false` | Enables `TreeCompleted`, `NodeStarted`, `NodeFinished`, and `BranchAborted` emission |
| `emit_blackboard_messages` | `bool` | `false` | Enables `BlackboardValueChanged` emission from dirty blackboard writes |
| `trace_capacity` | `usize` | `64` | Maximum number of entries stored in `BehaviorTreeTrace` |

## Tick Modes

| Variant | Meaning |
| --- | --- |
| `EveryFrame` | Tree evaluates every update while enabled |
| `Interval { seconds, phase_offset }` | Tree sleeps until the next scheduled interval; `phase_offset` offsets the first automatic tick so crowds can stagger their startup cadence |
| `Manual` | Tree performs its initial activation tick, then sleeps until it receives a wake request or another invalidation source |

## Abort Policies

`AbortPolicy` controls reactive reevaluation:

| Variant | Meaning |
| --- | --- |
| `None` | No reactive abort monitoring |
| `SelfOnly` | Reevaluate the currently running branch and abort it if its own guard fails |
| `LowerPriority` | Allow a higher-priority branch to abort a lower-priority running sibling |
| `Both` | Enable both behaviors |

These policies are used by reactive selectors and by decorators such as `Guard` and `BlackboardCondition`.

## Parallel Policies

`ParallelPolicy` defines when a parallel parent resolves and whether running siblings should be aborted once that threshold is met.

| Field | Type | Meaning |
| --- | --- | --- |
| `success` | `ParallelThreshold` | Success threshold: `Any`, `All`, or `AtLeast(n)` |
| `failure` | `ParallelThreshold` | Failure threshold: `Any`, `All`, or `AtLeast(n)` |
| `abort_running_siblings` | `bool` | Whether still-running children receive abort when the parent resolves |

Helpers:

- `ParallelPolicy::all_success_any_failure()`
- `ParallelPolicy::any_success_all_failure()`

## Decorator Semantics

The following decorators affect `Running` children in specific ways:

| Decorator | Running-child behavior |
| --- | --- |
| `Timeout { seconds }` | Fails the decorator when the child outlives the timeout; abort callback is invoked if the child was running |
| `Cooldown { seconds }` | Denies re-entry until the cooldown expires; it does not start the child during the cooldown window |
| `Repeater { limit }` | Re-enters the child after `Success` / `Failure` until the optional limit is reached |
| `Retry { attempts }` | Re-enters only after `Failure`, up to the configured attempt count |
| `UntilSuccess { limit }` | Keeps retrying while the child fails; stops on success or optional limit exhaustion |
| `UntilFailure { limit }` | Keeps retrying while the child succeeds; stops on failure or optional limit exhaustion |
| `Limiter { limit }` | Prevents further successful child entries once the limit is reached |
| `Guard { abort_policy, .. }` | Reevaluates the condition and may abort the child based on `AbortPolicy` |
| `BlackboardCondition { abort_policy, .. }` | Same as `Guard`, but driven by a typed blackboard condition instead of a handler |
| `Delay { seconds }` | Waits before first child entry; no child work happens until the delay elapses |
| `RunOnce { completed_status }` | Runs once, then returns the stored completed status on later entries |

## Services

Services are configured per branch with `ServiceBinding`.

| Field | Type | Default | Effect |
| --- | --- | --- | --- |
| `name` | `String` | required | Human-readable debug name |
| `key` | `ServiceKey` | required | Stable registered handler key |
| `interval_seconds` | `f32` | required | Minimum delay between service ticks |
| `start_immediately` | `bool` | `true` | Run on branch entry instead of waiting for the first interval |
| `wake_on_change` | `bool` | `true` | Mark the tree dirty when the service changes watched data |
| `watch_keys` | `Vec<BlackboardKeyId>` | empty | Blackboard keys relevant to the service for wake/debug purposes |

## Blackboard Schema

Keys are declared during build time and carry explicit metadata.

| Field | Meaning |
| --- | --- |
| `name` | Human-readable key name used in debug output and subtree remapping |
| `value_type` | Required value type |
| `direction` | `Input`, `Output`, `InOut`, or `Local` |
| `required` | Documents whether the key is expected to exist for the tree |
| `default_value` | Optional startup value inserted into new blackboards |
| `description` | Free-form metadata for future tooling |

Runtime effects:

- writes with a mismatched type are rejected
- changed writes bump per-key and total revisions
- dirty keys and `recent_changes` are recorded for one frame

## Debug Types

### `BehaviorTreeDebugRender`

Attach this component to an agent to draw line-based runtime feedback.

| Field | Default | Effect |
| --- | --- | --- |
| `ring_radius` | `0.8` | Radius of the root status ring |
| `vertical_spacing` | `0.18` | Spacing between active-path line segments |
| `target_entity_key` | `None` | Optional blackboard key used to draw a line to a target entity |

### `BehaviorTreeDebugFilter`

Optional resource that narrows which agents render debug lines.

| Field | Meaning |
| --- | --- |
| `entity` | Only render one specific controlled entity |
| `definition` | Only render instances using a specific definition |
| `tag` | Only render agents whose active path includes a node with this tag |

### `BehaviorTreeDebugGizmos`

This `GizmoConfigGroup` is opt-in. The runtime stays safe under `MinimalPlugins`, so consumers must explicitly enable the group when they want gizmo lines:

```rust
use bevy::gizmos::prelude::AppGizmoBuilder;

app.init_gizmo_group::<BehaviorTreeDebugGizmos>();
```

## Metrics And Trace

`BehaviorTreeInstance` always carries lightweight metrics and a bounded trace.

Key values:

- per-tree tick count
- per-node execution counts
- abort count
- service run count
- last tick duration
- recent `TraceKind` entries up to `trace_capacity`

Verbose message streams are still opt-in through `BehaviorTreeConfig`.

## Control Messages

| Message | Use |
| --- | --- |
| `TreeWakeRequested` | Wake a manual or sleeping tree because some external subsystem knows work is ready |
| `TreeResetRequested` | Reset the runtime state for an entity's current definition |
| `ActionResolution` | Complete a previously running async action ticket |

These messages enter the runtime during `Prepare`, not immediately at send time.
