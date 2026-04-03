# Architecture

`saddle-ai-behavior-tree` is split into two layers:

- Static definition:
  immutable tree shape, stable node IDs, blackboard schema, watch-key lists, services, tags, and subtree remapping results.
- Runtime instance:
  entity-local execution state, timers, counters, metrics, trace buffer, active path, wake flags, and blackboard contents.

This keeps definitions reusable across many entities while runtime state stays compact and local to the controlled entity.

## Update Pipeline

```text
Prepare
    -> Evaluate
    -> Apply
    -> Cleanup
    -> DebugRender
```

The public sets map to the following responsibilities:

- `Prepare`
  ingests external control messages, applies async completions, aligns runtime storage with the current definition, and converts watched-key changes into wake requests.
- `Evaluate`
  runs tree traversal for agents that should tick this frame according to `TickMode`, interval staggering, or explicit wakeups.
- `Apply`
  flushes buffered runtime messages such as `NodeStarted`, `NodeFinished`, and `BranchAborted`.
- `Cleanup`
  clears one-frame dirty state after downstream readers had a chance to inspect it.
- `DebugRender`
  draws opt-in gizmos when the app has initialized `BehaviorTreeDebugGizmos`.

## Definition Model

`BehaviorTreeDefinition` stores a flat node array rather than a recursive runtime structure.

- Nodes are indexed by `NodeId`.
- Child relationships are stored as dense `Vec<NodeId>` ranges.
- Definition data contains names, paths, tags, services, watch keys, and blackboard schema declarations.
- Leaves point to stable handler keys instead of embedded closures.

The result is shareable, inspectable, and compatible with future serialized authoring.

## Asset Definitions

`BehaviorTreeDefinitionAssetLoader` feeds serialized `.bt.ron` trees into the same `BehaviorTreeLibrary` used by Rust-authored definitions. Asset loading therefore changes authoring flow, not runtime semantics: once registered, loaded trees use the same flat-node layout, watch-key preprocessing, and handler indirection.

## Runtime Model

Each controlled entity owns:

- `BehaviorTreeAgent`
  definition selection, config, enable flag
- `BehaviorTreeInstance`
  active path, overall status, per-node runtime slots, counters, trace, wake state, and metrics
- `BehaviorTreeBlackboard`
  typed values, per-key revisions, dirty keys, and recent changes

The per-node slot array mirrors the definition array by index. That gives deterministic, allocation-light access for:

- sequence / selector cursors
- timeout / cooldown / delay timing
- repeater / limiter counters
- service next-due timestamps
- node-local memory
- async tickets and completions

## Node Lifecycle

Leaves and decorators follow an explicit lifecycle:

1. Enter / start
2. Tick
3. Finish with `Success`, `Failure`, or `Running`
4. Abort when a reactive parent or timeout/cooldown rule cancels the branch
5. Reset when the tree is reset or the definition changes

Stateful actions expose this directly through:

- `ActionHandler::stateful(on_start, on_tick, on_abort)`

Long-running leaves therefore get a real abort callback instead of being dropped silently.

## Blackboard Model

The blackboard is typed and schema-driven.

- Keys are declared up front with direction metadata: `Input`, `Output`, `InOut`, `Local`.
- Values are stored in a compact `BlackboardValue` enum.
- Writes increment per-key revisions and a total revision counter.
- Dirty keys and `recent_changes` support wakeups and optional developer-facing message streams.

Supported value types in v0.1:

- `bool`
- `i32`
- `f32`
- `Entity`
- `Vec2`
- `Vec3`
- `Quat`
- `String`

## Scoped Subtrees And Remapping

The crate uses a flat runtime blackboard plus build-time remapping instead of a dynamic scope stack.

- Remapped keys reuse the parent key ID directly.
- Non-remapped subtree-local keys are copied into the parent definition with stable prefixed names.
- Watch lists and service key references are remapped during `inline_subtree`.

This gives the same practical result as scoped subtree ports while keeping runtime lookups flat and branch-local behavior explicit in the built definition.

## Reactive Wakeups

The runtime avoids blind full-tree reevaluation by combining three mechanisms:

- explicit `TreeWakeRequested` messages
- watched-key dirty tracking on the blackboard
- service-driven wakeups through `wake_on_change`

Definitions precompute `watched_keys`, so `Prepare` can cheaply decide whether a blackboard revision matters for a given entity before forcing a reevaluation.

## Abort Flow

Abort behavior is explicit rather than emergent.

- Reactive selectors use `AbortPolicy` to decide whether they monitor themselves, lower-priority siblings, or both.
- Decorator guards and blackboard-condition decorators can invalidate a running child during reevaluation.
- Parallel parents can abort still-running siblings when the configured success/failure threshold resolves the parent.
- Aborting a running action calls its registered `on_abort` callback immediately.

`BranchAborted` and trace entries make the reason observable after the fact.

## Async Actions

Async completion is modeled through tickets instead of hidden task ownership.

- A running action asks for an `ActionTicket`.
- External code performs the background or delayed work.
- Completion is reported through `ActionResolution`.
- `Prepare` matches the ticket back onto the correct running node and wakes the tree.

This keeps ownership explicit and composes cleanly with Bevy tasks, channels, or non-task delayed workflows.

## Services

Services are attached to nodes, not run globally.

- Each `ServiceBinding` carries its own interval, watch keys, and wake policy.
- Service due-times are stored in the owning node's runtime slot.
- Services can update blackboard state or node-local memory without coupling the crate to any specific sensing or navigation stack.

This follows the same “periodic sensors, explicit branches” design used by production BT systems in Unreal and LimboAI.

## Debug And Inspection

Every instance carries runtime data that is useful in BRP and tests:

- `active_path`
- `status`
- `last_running_leaf`
- `last_abort_reason`
- `wake_reason`
- per-node execution counts
- per-tree metrics
- bounded `BehaviorTreeTrace`

Gizmo rendering is intentionally optional. The core runtime does not assume render/asset plugins are present, so headless tests and `MinimalPlugins` apps remain valid.

## Allocation Strategy

The runtime is designed to avoid per-tick churn:

- definitions are shared and immutable
- node runtime slots are preallocated per entity
- metrics arrays are sized from node count
- subtree remapping happens at build time
- handler lookup is explicit and stable
- cleanup reuses blackboard storage rather than recreating it every frame

The remaining intentional dynamic storage is:

- trace ring-buffer entries up to configured capacity
- message buffers before `Apply`
- one-time definition construction and subtree inlining

## Why This Shape

This design favors:

- deterministic runtime behavior
- cheap per-agent storage
- reusable definitions
- explicit data flow
- abort-safe multi-frame work
- observability in tests, BRP, and debug overlays

It leaves room for future asset-based authoring without requiring a serialized editor format in v0.1.
