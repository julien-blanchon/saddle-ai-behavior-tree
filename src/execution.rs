use std::time::Instant;

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;

use crate::blackboard::BehaviorTreeBlackboard;
use crate::components::BehaviorTreeAgent;
use crate::debug::{BehaviorTreeTraceEntry, TraceKind};
use crate::definition::{BehaviorTreeDefinition, NodeId};
use crate::handlers::{ActionContext, ConditionContext, ServiceContext};
use crate::messages::{
    BlackboardValueChanged, BranchAborted, NodeFinished, NodeStarted, TreeCompleted,
};
use crate::nodes::{
    BehaviorStatus, DecoratorKind, NodeKind, ParallelPolicy, ParallelThreshold, SelectorKind,
    SequenceKind,
};
use crate::resources::{BehaviorTreeHandlers, RuntimeMessageBuffer};
use crate::runtime::{
    BehaviorTreeConfig, BehaviorTreeInstance, BehaviorTreeRunState, NodeRuntimeState, TickMode,
};

pub(crate) fn should_wake_for_blackboard(
    definition: &BehaviorTreeDefinition,
    blackboard: &BehaviorTreeBlackboard,
) -> bool {
    blackboard
        .dirty_keys
        .iter()
        .any(|key| definition.watched_keys.is_empty() || definition.watched_keys.contains(key))
}

pub(crate) fn should_tick(
    config: &BehaviorTreeConfig,
    instance: &BehaviorTreeInstance,
    now: f32,
) -> bool {
    if instance.wake_requested {
        return true;
    }
    if matches!(
        instance.status,
        BehaviorTreeRunState::Success | BehaviorTreeRunState::Failure
    ) && !config.restart_on_completion
    {
        return false;
    }
    match config.tick_mode {
        TickMode::EveryFrame => true,
        TickMode::Interval { .. } => now >= instance.next_tick_at,
        TickMode::Manual => false,
    }
}

pub(crate) fn schedule_next_tick(
    config: &BehaviorTreeConfig,
    instance: &mut BehaviorTreeInstance,
    now: f32,
) {
    instance.next_tick_at = match config.tick_mode {
        TickMode::EveryFrame | TickMode::Manual => now,
        TickMode::Interval {
            seconds,
            phase_offset: _,
        } => now + seconds.max(0.001),
    };
}

struct ExecutionCtx<'a> {
    world: &'a mut World,
    entity: Entity,
    frame: u64,
    now: f32,
    definition: &'a BehaviorTreeDefinition,
    handlers: &'a BehaviorTreeHandlers,
    messages: &'a mut RuntimeMessageBuffer,
    config: &'a BehaviorTreeConfig,
    instance: &'a mut BehaviorTreeInstance,
    blackboard: &'a mut BehaviorTreeBlackboard,
    active_path: Vec<NodeId>,
}

impl<'a> ExecutionCtx<'a> {
    fn push_trace(
        &mut self,
        node: NodeId,
        kind: TraceKind,
        status: Option<BehaviorStatus>,
        note: impl Into<String>,
    ) {
        self.instance.trace.push(BehaviorTreeTraceEntry {
            frame: self.frame,
            node,
            kind,
            status,
            note: note.into(),
        });
    }

    fn start_node(&mut self, node: NodeId) {
        let runtime = &mut self.instance.node_states[node.0 as usize];
        runtime.status = BehaviorTreeRunState::Running;
        runtime.entered_at = self.now;
        runtime.execution_count += 1;
        self.instance.metrics.node_start_count += 1;
        self.instance.metrics.node_execution_counts[node.0 as usize] += 1;
        if self.config.emit_lifecycle_messages {
            let path = self
                .definition
                .node(node)
                .map(|node| node.path.clone())
                .unwrap_or_else(|| format!("node:{}", node.0));
            self.messages.node_started.push(NodeStarted {
                entity: self.entity,
                node,
                path,
            });
        }
        self.push_trace(node, TraceKind::Started, None, "");
    }

    fn set_running(&mut self, node: NodeId) -> BehaviorStatus {
        let runtime = &mut self.instance.node_states[node.0 as usize];
        runtime.status = BehaviorTreeRunState::Running;
        BehaviorStatus::Running
    }

    fn finish_node(&mut self, node: NodeId, status: BehaviorStatus) -> BehaviorStatus {
        let runtime = &mut self.instance.node_states[node.0 as usize];
        runtime.status = match status {
            BehaviorStatus::Success => BehaviorTreeRunState::Success,
            BehaviorStatus::Failure => BehaviorTreeRunState::Failure,
            BehaviorStatus::Running => BehaviorTreeRunState::Running,
        };
        runtime.finished_at = self.now;
        runtime.last_result = Some(status);
        runtime.async_resolution = None;
        self.instance.metrics.node_finish_count += 1;
        if self.config.emit_lifecycle_messages && status != BehaviorStatus::Running {
            let path = self
                .definition
                .node(node)
                .map(|node| node.path.clone())
                .unwrap_or_else(|| format!("node:{}", node.0));
            self.messages.node_finished.push(NodeFinished {
                entity: self.entity,
                node,
                path,
                status,
            });
        }
        self.push_trace(node, TraceKind::Finished, Some(status), "");
        status
    }

    fn reset_subtree(&mut self, node: NodeId) {
        if let Some(definition) = self.definition.node(node) {
            for child in &definition.children {
                self.reset_subtree(*child);
            }
        }
        let runtime = &mut self.instance.node_states[node.0 as usize];
        *runtime = NodeRuntimeState::default();
    }

    fn abort_subtree(&mut self, node: NodeId, reason: impl Into<String>, emit_root_message: bool) {
        let reason = reason.into();
        let definition = match self.definition.node(node).cloned() {
            Some(node) => node,
            None => return,
        };
        for child in &definition.children {
            self.abort_subtree(*child, reason.clone(), false);
        }
        if let NodeKind::Action(action_key) = definition.kind {
            let handler = self.handlers.action(&action_key).cloned();
            let was_running = matches!(
                self.instance.node_states[node.0 as usize].status,
                BehaviorTreeRunState::Running
            );
            if was_running {
                if let Some(handler) = handler {
                    let mut action_context = ActionContext {
                        world: self.world,
                        entity: self.entity,
                        definition: self.definition,
                        blackboard: self.blackboard,
                        node_id: node,
                        node_state: &mut self.instance.node_states[node.0 as usize],
                        action_ticket_counter: &mut self.instance.action_ticket_counter,
                        wake_requested: &mut self.instance.wake_requested,
                        wake_reason: &mut self.instance.wake_reason,
                    };
                    if let Some(on_abort) = handler.on_abort {
                        on_abort(&mut action_context);
                    }
                }
            }
        }
        self.instance.metrics.abort_count += 1;
        self.instance.last_abort_reason = reason.clone();
        self.push_trace(node, TraceKind::Aborted, None, reason.clone());
        if emit_root_message && self.config.emit_lifecycle_messages {
            self.messages.branch_aborted.push(BranchAborted {
                entity: self.entity,
                node,
                path: definition.path.clone(),
                reason,
            });
        }
        self.reset_subtree(node);
    }

    fn run_services(&mut self, node: NodeId) {
        let definition = match self.definition.node(node).cloned() {
            Some(definition) => definition,
            None => return,
        };
        if definition.services.is_empty() {
            return;
        }
        if self.instance.node_states[node.0 as usize]
            .service_due_at
            .len()
            != definition.services.len()
        {
            self.instance.node_states[node.0 as usize].service_due_at = definition
                .services
                .iter()
                .map(|service| {
                    if service.start_immediately {
                        self.now
                    } else {
                        self.now + service.interval_seconds.max(0.001)
                    }
                })
                .collect();
        }
        for (index, service) in definition.services.iter().enumerate() {
            let due_at = self.instance.node_states[node.0 as usize].service_due_at[index];
            if self.now < due_at {
                continue;
            }
            let handler = self.handlers.service(&service.key).cloned();
            if let Some(handler) = handler {
                let previous_revision = self.blackboard.total_revision;
                let mut context = ServiceContext {
                    world: self.world,
                    entity: self.entity,
                    definition: self.definition,
                    blackboard: self.blackboard,
                    node_id: node,
                    node_state: &mut self.instance.node_states[node.0 as usize],
                    wake_requested: &mut self.instance.wake_requested,
                    wake_reason: &mut self.instance.wake_reason,
                };
                (handler.tick)(&mut context);
                if service.wake_on_change && self.blackboard.total_revision != previous_revision {
                    self.instance.wake_requested = true;
                    self.instance.wake_reason = format!("service '{}'", service.name);
                }
            }
            self.instance.metrics.service_run_count += 1;
            self.push_trace(node, TraceKind::Service, None, service.name.clone());
            self.instance.node_states[node.0 as usize].service_due_at[index] =
                self.now + service.interval_seconds.max(0.001);
        }
    }
}

pub(crate) fn tick_entity(
    world: &mut World,
    entity: Entity,
    agent: &BehaviorTreeAgent,
    definition: &BehaviorTreeDefinition,
    handlers: &BehaviorTreeHandlers,
    message_buffer: &mut RuntimeMessageBuffer,
) {
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed_secs);
    let frame = world
        .get_resource::<FrameCount>()
        .map_or(0_u64, |frame| u64::from(frame.0));

    let Some(mut instance) = world.entity_mut(entity).take::<BehaviorTreeInstance>() else {
        return;
    };
    let Some(mut blackboard) = world.entity_mut(entity).take::<BehaviorTreeBlackboard>() else {
        world.entity_mut(entity).insert(instance);
        return;
    };

    {
        let started = Instant::now();
        let mut exec = ExecutionCtx {
            world,
            entity,
            frame,
            now,
            definition,
            handlers,
            messages: message_buffer,
            config: &agent.config,
            instance: &mut instance,
            blackboard: &mut blackboard,
            active_path: Vec::new(),
        };
        exec.instance.metrics.tick_count += 1;
        exec.instance.last_running_leaf = None;
        exec.instance.wake_requested = false;
        exec.instance.wake_reason.clear();

        let result = tick_node(&mut exec, definition.root);
        exec.active_path.reverse();
        exec.instance.active_path = exec.active_path;
        exec.instance.status = match result {
            BehaviorStatus::Success => BehaviorTreeRunState::Success,
            BehaviorStatus::Failure => BehaviorTreeRunState::Failure,
            BehaviorStatus::Running => BehaviorTreeRunState::Running,
        };
        if result != BehaviorStatus::Running && agent.config.emit_lifecycle_messages {
            exec.messages.tree_completed.push(TreeCompleted {
                entity,
                definition: agent.definition,
                status: result,
            });
        }
        schedule_next_tick(&agent.config, exec.instance, now);
        exec.instance.observed_blackboard_revision = exec.blackboard.total_revision;
        exec.instance.metrics.last_tick_micros = started.elapsed().as_micros() as u64;

        if agent.config.emit_blackboard_messages {
            for change in exec.blackboard.take_recent_changes() {
                exec.messages
                    .blackboard_changed
                    .push(BlackboardValueChanged {
                        entity,
                        key: change.key,
                        name: change.name,
                        revision: change.revision,
                        old_value: change.old_value,
                        new_value: change.new_value,
                    });
            }
        } else {
            exec.blackboard.recent_changes.clear();
        }
    }

    world.entity_mut(entity).insert((instance, blackboard));
}

fn tick_node(exec: &mut ExecutionCtx<'_>, node_id: NodeId) -> BehaviorStatus {
    let node = exec
        .definition
        .node(node_id)
        .cloned()
        .expect("node must exist in definition");

    exec.run_services(node_id);

    match node.kind {
        NodeKind::Action(action_key) => tick_action(exec, node_id, &action_key),
        NodeKind::Condition { key, .. } => tick_condition(exec, node_id, &key),
        NodeKind::Sequence(kind) => tick_sequence(exec, node_id, kind, &node.children),
        NodeKind::Selector(kind) => tick_selector(exec, node_id, kind, &node.children),
        NodeKind::Parallel(policy) => tick_parallel(exec, node_id, policy, &node.children),
        NodeKind::Decorator(decorator) => {
            tick_decorator(exec, node_id, decorator, node.children[0])
        }
    }
}

fn tick_action(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    action_key: &crate::handlers::ActionKey,
) -> BehaviorStatus {
    let handler = match exec.handlers.action(action_key).cloned() {
        Some(handler) => handler,
        None => {
            return exec.finish_node(node_id, BehaviorStatus::Failure);
        }
    };
    let restart = matches!(
        exec.instance.node_states[node_id.0 as usize].status,
        BehaviorTreeRunState::Success | BehaviorTreeRunState::Failure | BehaviorTreeRunState::Idle
    );
    if restart {
        exec.reset_subtree(node_id);
        exec.start_node(node_id);
    }
    let status = {
        let mut context = ActionContext {
            world: exec.world,
            entity: exec.entity,
            definition: exec.definition,
            blackboard: exec.blackboard,
            node_id,
            node_state: &mut exec.instance.node_states[node_id.0 as usize],
            action_ticket_counter: &mut exec.instance.action_ticket_counter,
            wake_requested: &mut exec.instance.wake_requested,
            wake_reason: &mut exec.instance.wake_reason,
        };
        if restart {
            (handler.on_start)(&mut context)
        } else {
            (handler.on_tick)(&mut context)
        }
    };
    match status {
        BehaviorStatus::Running => {
            exec.instance.last_running_leaf = Some(node_id);
            exec.active_path.push(node_id);
            exec.set_running(node_id)
        }
        terminal => exec.finish_node(node_id, terminal),
    }
}

fn tick_condition(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    condition_key: &crate::handlers::ConditionKey,
) -> BehaviorStatus {
    exec.start_node(node_id);
    let result = if let Some(handler) = exec.handlers.condition(condition_key).cloned() {
        let mut context = ConditionContext {
            world: exec.world,
            entity: exec.entity,
            definition: exec.definition,
            blackboard: exec.blackboard,
            node_id,
        };
        (handler.evaluate)(&mut context)
    } else {
        false
    };
    exec.finish_node(
        node_id,
        if result {
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        },
    )
}

fn tick_sequence(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    kind: SequenceKind,
    children: &[NodeId],
) -> BehaviorStatus {
    if !matches!(
        exec.instance.node_states[node_id.0 as usize].status,
        BehaviorTreeRunState::Running
    ) {
        exec.start_node(node_id);
    }
    let previous_cursor = exec.instance.node_states[node_id.0 as usize].cursor;
    let start_index = match kind {
        SequenceKind::Sequence | SequenceKind::SequenceWithMemory => previous_cursor,
        SequenceKind::ReactiveSequence => 0,
    };
    for index in start_index..children.len() {
        let child = children[index];
        let restart_child = match kind {
            SequenceKind::ReactiveSequence => true,
            SequenceKind::Sequence => !matches!(
                exec.instance.node_states[child.0 as usize].status,
                BehaviorTreeRunState::Running
            ),
            SequenceKind::SequenceWithMemory => {
                index == previous_cursor
                    && !matches!(
                        exec.instance.node_states[child.0 as usize].status,
                        BehaviorTreeRunState::Running
                    )
            }
        };
        if restart_child {
            exec.reset_subtree(child);
        }
        let result = tick_node(exec, child);
        match result {
            BehaviorStatus::Success => continue,
            BehaviorStatus::Failure => {
                if matches!(kind, SequenceKind::ReactiveSequence) && previous_cursor > index {
                    exec.abort_subtree(
                        children[previous_cursor],
                        "reactive sequence aborted",
                        true,
                    );
                }
                exec.instance.node_states[node_id.0 as usize].cursor = match kind {
                    SequenceKind::SequenceWithMemory => index,
                    SequenceKind::Sequence | SequenceKind::ReactiveSequence => 0,
                };
                return exec.finish_node(node_id, BehaviorStatus::Failure);
            }
            BehaviorStatus::Running => {
                if matches!(kind, SequenceKind::ReactiveSequence)
                    && previous_cursor != index
                    && previous_cursor < children.len()
                {
                    let previous = children[previous_cursor];
                    if previous != child {
                        exec.abort_subtree(previous, "reactive sequence restart", true);
                    }
                }
                exec.instance.node_states[node_id.0 as usize].cursor = index;
                exec.active_path.push(node_id);
                return exec.set_running(node_id);
            }
        }
    }
    exec.instance.node_states[node_id.0 as usize].cursor = 0;
    exec.finish_node(node_id, BehaviorStatus::Success)
}

fn tick_selector(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    kind: SelectorKind,
    children: &[NodeId],
) -> BehaviorStatus {
    if !matches!(
        exec.instance.node_states[node_id.0 as usize].status,
        BehaviorTreeRunState::Running
    ) {
        exec.start_node(node_id);
    }
    let previous_cursor = exec.instance.node_states[node_id.0 as usize].cursor;
    let start_index = match kind {
        SelectorKind::Selector | SelectorKind::SelectorWithMemory => previous_cursor,
        SelectorKind::ReactiveSelector { abort_policy } => {
            if abort_policy.monitors_lower_priority() {
                0
            } else {
                previous_cursor
            }
        }
    };
    for index in start_index..children.len() {
        let child = children[index];
        let restart_child = match kind {
            SelectorKind::ReactiveSelector { .. } => true,
            SelectorKind::Selector => !matches!(
                exec.instance.node_states[child.0 as usize].status,
                BehaviorTreeRunState::Running
            ),
            SelectorKind::SelectorWithMemory => {
                index == previous_cursor
                    && !matches!(
                        exec.instance.node_states[child.0 as usize].status,
                        BehaviorTreeRunState::Running
                    )
            }
        };
        if restart_child {
            exec.reset_subtree(child);
        }
        let result = tick_node(exec, child);
        match result {
            BehaviorStatus::Failure => continue,
            BehaviorStatus::Success => {
                if let SelectorKind::ReactiveSelector { abort_policy } = kind
                    && abort_policy.monitors_lower_priority()
                    && previous_cursor != index
                    && previous_cursor < children.len()
                {
                    let previous = children[previous_cursor];
                    if previous != child {
                        exec.abort_subtree(previous, "lower priority branch aborted", true);
                    }
                }
                exec.instance.node_states[node_id.0 as usize].cursor = match kind {
                    SelectorKind::SelectorWithMemory => index,
                    SelectorKind::Selector | SelectorKind::ReactiveSelector { .. } => 0,
                };
                return exec.finish_node(node_id, BehaviorStatus::Success);
            }
            BehaviorStatus::Running => {
                if let SelectorKind::ReactiveSelector { abort_policy } = kind
                    && abort_policy.monitors_lower_priority()
                    && previous_cursor != index
                    && previous_cursor < children.len()
                {
                    let previous = children[previous_cursor];
                    if previous != child {
                        exec.abort_subtree(previous, "lower priority branch preempted", true);
                    }
                }
                exec.instance.node_states[node_id.0 as usize].cursor = index;
                exec.active_path.push(node_id);
                return exec.set_running(node_id);
            }
        }
    }
    exec.instance.node_states[node_id.0 as usize].cursor = 0;
    exec.finish_node(node_id, BehaviorStatus::Failure)
}

fn tick_parallel(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    policy: ParallelPolicy,
    children: &[NodeId],
) -> BehaviorStatus {
    if !matches!(
        exec.instance.node_states[node_id.0 as usize].status,
        BehaviorTreeRunState::Running
    ) {
        exec.start_node(node_id);
    }
    let mut successes = 0u16;
    let mut failures = 0u16;
    let mut running = false;
    for child in children {
        let child_status = match exec.instance.node_states[child.0 as usize].status {
            BehaviorTreeRunState::Success => BehaviorStatus::Success,
            BehaviorTreeRunState::Failure => BehaviorStatus::Failure,
            BehaviorTreeRunState::Running | BehaviorTreeRunState::Idle => tick_node(exec, *child),
            BehaviorTreeRunState::Deactivated => BehaviorStatus::Failure,
        };
        match child_status {
            BehaviorStatus::Success => successes += 1,
            BehaviorStatus::Failure => failures += 1,
            BehaviorStatus::Running => running = true,
        }
    }
    if threshold_met(policy.success, successes, children.len() as u16) {
        if policy.abort_running_siblings {
            for child in children {
                if matches!(
                    exec.instance.node_states[child.0 as usize].status,
                    BehaviorTreeRunState::Running
                ) {
                    exec.abort_subtree(*child, "parallel success threshold reached", true);
                }
            }
        }
        return exec.finish_node(node_id, BehaviorStatus::Success);
    }
    if threshold_met(policy.failure, failures, children.len() as u16) {
        if policy.abort_running_siblings {
            for child in children {
                if matches!(
                    exec.instance.node_states[child.0 as usize].status,
                    BehaviorTreeRunState::Running
                ) {
                    exec.abort_subtree(*child, "parallel failure threshold reached", true);
                }
            }
        }
        return exec.finish_node(node_id, BehaviorStatus::Failure);
    }
    if running {
        exec.active_path.push(node_id);
        exec.set_running(node_id)
    } else {
        exec.finish_node(node_id, BehaviorStatus::Failure)
    }
}

fn tick_decorator(
    exec: &mut ExecutionCtx<'_>,
    node_id: NodeId,
    decorator: DecoratorKind,
    child: NodeId,
) -> BehaviorStatus {
    if !matches!(
        exec.instance.node_states[node_id.0 as usize].status,
        BehaviorTreeRunState::Running
    ) {
        exec.start_node(node_id);
    }
    match decorator {
        DecoratorKind::Inverter => match tick_node(exec, child) {
            BehaviorStatus::Success => exec.finish_node(node_id, BehaviorStatus::Failure),
            BehaviorStatus::Failure => exec.finish_node(node_id, BehaviorStatus::Success),
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
        },
        DecoratorKind::ForceSuccess | DecoratorKind::Succeeder => match tick_node(exec, child) {
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
            _ => exec.finish_node(node_id, BehaviorStatus::Success),
        },
        DecoratorKind::ForceFailure => match tick_node(exec, child) {
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
            _ => exec.finish_node(node_id, BehaviorStatus::Failure),
        },
        DecoratorKind::Timeout { seconds } => {
            let result = tick_node(exec, child);
            if result == BehaviorStatus::Running
                && exec.now - exec.instance.node_states[node_id.0 as usize].entered_at >= seconds
            {
                exec.abort_subtree(child, "timeout", true);
                exec.finish_node(node_id, BehaviorStatus::Failure)
            } else if result == BehaviorStatus::Running {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            } else {
                exec.finish_node(node_id, result)
            }
        }
        DecoratorKind::Cooldown { seconds } => {
            if exec.now < exec.instance.node_states[node_id.0 as usize].cooldown_until
                && !matches!(
                    exec.instance.node_states[child.0 as usize].status,
                    BehaviorTreeRunState::Running
                )
            {
                return exec.finish_node(node_id, BehaviorStatus::Failure);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => {
                    exec.instance.node_states[node_id.0 as usize].cooldown_until =
                        exec.now + seconds;
                    exec.finish_node(node_id, terminal)
                }
            }
        }
        DecoratorKind::Retry { attempts } => match tick_node(exec, child) {
            BehaviorStatus::Success => {
                exec.instance.node_states[node_id.0 as usize].counter = 0;
                exec.finish_node(node_id, BehaviorStatus::Success)
            }
            BehaviorStatus::Failure => {
                let used = exec.instance.node_states[node_id.0 as usize].counter;
                if used < attempts {
                    exec.instance.node_states[node_id.0 as usize].counter += 1;
                    exec.reset_subtree(child);
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                } else {
                    exec.instance.node_states[node_id.0 as usize].counter = 0;
                    exec.finish_node(node_id, BehaviorStatus::Failure)
                }
            }
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
        },
        DecoratorKind::Repeater { limit } => match tick_node(exec, child) {
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
            _ => {
                let current = exec.instance.node_states[node_id.0 as usize].counter;
                if limit.is_some_and(|limit| current + 1 >= limit) {
                    exec.instance.node_states[node_id.0 as usize].counter = 0;
                    exec.finish_node(node_id, BehaviorStatus::Success)
                } else {
                    exec.instance.node_states[node_id.0 as usize].counter += 1;
                    exec.reset_subtree(child);
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
            }
        },
        DecoratorKind::UntilSuccess { limit } => match tick_node(exec, child) {
            BehaviorStatus::Success => {
                exec.instance.node_states[node_id.0 as usize].counter = 0;
                exec.finish_node(node_id, BehaviorStatus::Success)
            }
            BehaviorStatus::Failure => {
                let current = exec.instance.node_states[node_id.0 as usize].counter;
                if limit.is_some_and(|limit| current + 1 >= limit) {
                    exec.instance.node_states[node_id.0 as usize].counter = 0;
                    exec.finish_node(node_id, BehaviorStatus::Failure)
                } else {
                    exec.instance.node_states[node_id.0 as usize].counter += 1;
                    exec.reset_subtree(child);
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
            }
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
        },
        DecoratorKind::UntilFailure { limit } => match tick_node(exec, child) {
            BehaviorStatus::Failure => {
                exec.instance.node_states[node_id.0 as usize].counter = 0;
                exec.finish_node(node_id, BehaviorStatus::Success)
            }
            BehaviorStatus::Success => {
                let current = exec.instance.node_states[node_id.0 as usize].counter;
                if limit.is_some_and(|limit| current + 1 >= limit) {
                    exec.instance.node_states[node_id.0 as usize].counter = 0;
                    exec.finish_node(node_id, BehaviorStatus::Failure)
                } else {
                    exec.instance.node_states[node_id.0 as usize].counter += 1;
                    exec.reset_subtree(child);
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
            }
            BehaviorStatus::Running => {
                exec.active_path.push(node_id);
                exec.set_running(node_id)
            }
        },
        DecoratorKind::Limiter { limit } => {
            if exec.instance.node_states[node_id.0 as usize].limiter_used >= limit {
                return exec.finish_node(node_id, BehaviorStatus::Failure);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => {
                    exec.instance.node_states[node_id.0 as usize].limiter_used += 1;
                    exec.finish_node(node_id, terminal)
                }
            }
        }
        DecoratorKind::Guard {
            condition,
            abort_policy,
            watch_keys: _,
        } => {
            let predicate = if let Some(handler) = exec.handlers.condition(&condition).cloned() {
                let mut context = ConditionContext {
                    world: exec.world,
                    entity: exec.entity,
                    definition: exec.definition,
                    blackboard: exec.blackboard,
                    node_id,
                };
                (handler.evaluate)(&mut context)
            } else {
                false
            };
            let child_running = matches!(
                exec.instance.node_states[child.0 as usize].status,
                BehaviorTreeRunState::Running
            );
            if !predicate {
                if child_running && abort_policy.monitors_self() {
                    exec.abort_subtree(child, "guard failed", true);
                } else if child_running {
                    match tick_node(exec, child) {
                        BehaviorStatus::Running => {
                            exec.active_path.push(node_id);
                            return exec.set_running(node_id);
                        }
                        terminal => return exec.finish_node(node_id, terminal),
                    }
                }
                return exec.finish_node(node_id, BehaviorStatus::Failure);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => exec.finish_node(node_id, terminal),
            }
        }
        DecoratorKind::Delay { seconds } => {
            if exec.instance.node_states[node_id.0 as usize]
                .delay_until
                .is_none()
            {
                exec.instance.node_states[node_id.0 as usize].delay_until =
                    Some(exec.now + seconds);
            }
            if exec.now
                < exec.instance.node_states[node_id.0 as usize]
                    .delay_until
                    .unwrap_or_default()
            {
                exec.active_path.push(node_id);
                return exec.set_running(node_id);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => {
                    exec.instance.node_states[node_id.0 as usize].delay_until = None;
                    exec.finish_node(node_id, terminal)
                }
            }
        }
        DecoratorKind::RunOnce { completed_status } => {
            if exec.instance.node_states[node_id.0 as usize].counter > 0 {
                return exec.finish_node(node_id, completed_status);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => {
                    exec.instance.node_states[node_id.0 as usize].counter = 1;
                    exec.finish_node(node_id, terminal)
                }
            }
        }
        DecoratorKind::BlackboardCondition {
            key,
            condition,
            abort_policy,
        } => {
            let child_running = matches!(
                exec.instance.node_states[child.0 as usize].status,
                BehaviorTreeRunState::Running
            );
            if !condition.evaluate(exec.blackboard.value(key)) {
                if child_running && abort_policy.monitors_self() {
                    exec.abort_subtree(child, "blackboard condition failed", true);
                } else if child_running {
                    match tick_node(exec, child) {
                        BehaviorStatus::Running => {
                            exec.active_path.push(node_id);
                            return exec.set_running(node_id);
                        }
                        terminal => return exec.finish_node(node_id, terminal),
                    }
                }
                return exec.finish_node(node_id, BehaviorStatus::Failure);
            }
            match tick_node(exec, child) {
                BehaviorStatus::Running => {
                    exec.active_path.push(node_id);
                    exec.set_running(node_id)
                }
                terminal => exec.finish_node(node_id, terminal),
            }
        }
    }
}

fn threshold_met(threshold: ParallelThreshold, count: u16, child_count: u16) -> bool {
    match threshold {
        ParallelThreshold::Any => count > 0,
        ParallelThreshold::All => count == child_count,
        ParallelThreshold::AtLeast(required) => count >= required,
    }
}

#[cfg(test)]
#[path = "semantics_tests.rs"]
mod semantics_tests;

#[cfg(test)]
#[path = "perf_tests.rs"]
mod perf_tests;
