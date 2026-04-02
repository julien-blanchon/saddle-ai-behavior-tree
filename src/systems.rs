use bevy::prelude::*;
use bevy::{ecs::system::SystemState, gizmos::gizmos::GizmoStorage};

use crate::blackboard::BehaviorTreeBlackboard;
use crate::components::BehaviorTreeAgent;
use crate::debug::{BehaviorTreeDebugFilter, BehaviorTreeDebugGizmos, BehaviorTreeDebugRender};
use crate::execution::{should_tick, should_wake_for_blackboard, tick_entity};
use crate::resources::{
    BehaviorTreeHandlers, BehaviorTreeLibrary, ControlInbox, RuntimeMessageBuffer,
};
use crate::runtime::{BehaviorTreeInstance, BehaviorTreeRunState, TickMode};

pub(crate) fn activate_agents(world: &mut World) {
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<BehaviorTreeAgent>>()
        .iter(world)
        .collect();
    for entity in entities {
        ensure_instance(world, entity);
    }
}

pub(crate) fn deactivate_agents(world: &mut World) {
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, (With<BehaviorTreeAgent>, With<BehaviorTreeInstance>)>()
        .iter(world)
        .collect();
    for entity in entities {
        world.entity_mut(entity).remove::<BehaviorTreeInstance>();
        if let Some(mut blackboard) = world.get_mut::<BehaviorTreeBlackboard>(entity) {
            blackboard.dirty_keys.clear();
            blackboard.recent_changes.clear();
        }
    }
}

pub(crate) fn ingest_control_messages(
    mut inbox: ResMut<ControlInbox>,
    mut wake_requests: MessageReader<crate::messages::TreeWakeRequested>,
    mut reset_requests: MessageReader<crate::messages::TreeResetRequested>,
    mut action_resolutions: MessageReader<crate::messages::ActionResolution>,
) {
    inbox.wake_requests.extend(wake_requests.read().cloned());
    inbox.reset_requests.extend(reset_requests.read().cloned());
    inbox
        .action_resolutions
        .extend(action_resolutions.read().cloned());
}

pub(crate) fn prepare_agents(world: &mut World) {
    let (wake_requests, reset_requests, action_resolutions) = {
        let mut inbox = world.resource_mut::<ControlInbox>();
        (
            std::mem::take(&mut inbox.wake_requests),
            std::mem::take(&mut inbox.reset_requests),
            std::mem::take(&mut inbox.action_resolutions),
        )
    };

    for wake in wake_requests {
        if let Some(mut instance) = world.get_mut::<BehaviorTreeInstance>(wake.entity) {
            instance.wake_requested = true;
            instance.wake_reason = wake.reason;
        }
    }

    for reset in reset_requests {
        if let Some(mut instance) = world.get_mut::<BehaviorTreeInstance>(reset.entity) {
            instance.status = BehaviorTreeRunState::Idle;
            instance.active_path.clear();
            instance.last_running_leaf = None;
            instance.wake_requested = true;
            instance.wake_reason = reset.reason;
            for node_state in &mut instance.node_states {
                *node_state = Default::default();
            }
        }
    }

    for resolution in action_resolutions {
        if let Some(mut instance) = world.get_mut::<BehaviorTreeInstance>(resolution.entity) {
            for node_state in &mut instance.node_states {
                if node_state.async_ticket == Some(resolution.ticket) {
                    node_state.async_resolution = Some(resolution.status);
                    instance.wake_requested = true;
                    instance.wake_reason = "async action resolved".to_owned();
                    break;
                }
            }
        }
    }

    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<BehaviorTreeAgent>>()
        .iter(world)
        .collect();
    for entity in entities {
        ensure_instance(world, entity);
    }

    let library = world.resource::<BehaviorTreeLibrary>().clone();
    let agents: Vec<(Entity, BehaviorTreeAgent)> = world
        .query::<(Entity, &BehaviorTreeAgent)>()
        .iter(world)
        .map(|(entity, agent)| (entity, agent.clone()))
        .collect();
    for (entity, agent) in agents {
        if !agent.enabled {
            continue;
        }
        let Some(definition) = library.get(agent.definition) else {
            continue;
        };
        let blackboard_revision = world
            .get::<BehaviorTreeBlackboard>(entity)
            .map(|blackboard| {
                (
                    blackboard.total_revision,
                    should_wake_for_blackboard(definition, blackboard),
                )
            });
        if let Some((revision, should_wake)) = blackboard_revision
            && should_wake
            && let Some(mut instance) = world.get_mut::<BehaviorTreeInstance>(entity)
            && revision != instance.observed_blackboard_revision
        {
            instance.wake_requested = true;
            instance.wake_reason = "blackboard changed".to_owned();
        }
    }
}

pub(crate) fn evaluate_agents(world: &mut World) {
    let library = world.resource::<BehaviorTreeLibrary>().clone();
    let handlers = world.resource::<BehaviorTreeHandlers>().clone();
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed_secs);
    let entities: Vec<(Entity, BehaviorTreeAgent)> = world
        .query::<(Entity, &BehaviorTreeAgent)>()
        .iter(world)
        .map(|(entity, agent)| (entity, agent.clone()))
        .collect();
    let mut message_buffer = std::mem::take(&mut *world.resource_mut::<RuntimeMessageBuffer>());
    for (entity, agent) in entities {
        if !agent.enabled {
            continue;
        }
        let Some(instance) = world.get::<BehaviorTreeInstance>(entity) else {
            continue;
        };
        let Some(definition) = library.get(agent.definition) else {
            continue;
        };
        if should_tick(&agent.config, instance, now) {
            tick_entity(
                world,
                entity,
                &agent,
                definition,
                &handlers,
                &mut message_buffer,
            );
        }
    }
    *world.resource_mut::<RuntimeMessageBuffer>() = message_buffer;
}

pub(crate) fn flush_runtime_messages(
    mut buffer: ResMut<RuntimeMessageBuffer>,
    mut tree_completed: MessageWriter<crate::messages::TreeCompleted>,
    mut node_started: MessageWriter<crate::messages::NodeStarted>,
    mut node_finished: MessageWriter<crate::messages::NodeFinished>,
    mut branch_aborted: MessageWriter<crate::messages::BranchAborted>,
    mut blackboard_changed: MessageWriter<crate::messages::BlackboardValueChanged>,
) {
    for message in buffer.tree_completed.drain(..) {
        tree_completed.write(message);
    }
    for message in buffer.node_started.drain(..) {
        node_started.write(message);
    }
    for message in buffer.node_finished.drain(..) {
        node_finished.write(message);
    }
    for message in buffer.branch_aborted.drain(..) {
        branch_aborted.write(message);
    }
    for message in buffer.blackboard_changed.drain(..) {
        blackboard_changed.write(message);
    }
}

pub(crate) fn cleanup_agents(mut query: Query<(&BehaviorTreeAgent, &mut BehaviorTreeBlackboard)>) {
    for (agent, mut blackboard) in &mut query {
        if !agent.config.emit_blackboard_messages {
            blackboard.recent_changes.clear();
        }
        blackboard.dirty_keys.clear();
    }
}

type DebugRenderState<'w, 's> = SystemState<(
    Option<Res<'w, BehaviorTreeDebugFilter>>,
    Res<'w, BehaviorTreeLibrary>,
    Gizmos<'w, 's, BehaviorTreeDebugGizmos>,
    Query<
        'w,
        's,
        (
            Entity,
            &'static BehaviorTreeAgent,
            &'static BehaviorTreeInstance,
            &'static BehaviorTreeBlackboard,
            &'static BehaviorTreeDebugRender,
            Option<&'static GlobalTransform>,
        ),
    >,
    Query<'w, 's, &'static GlobalTransform>,
)>;

pub(crate) fn debug_render(world: &mut World) {
    if !world.contains_resource::<GizmoStorage<BehaviorTreeDebugGizmos, ()>>() {
        return;
    }

    let mut state: DebugRenderState<'_, '_> = SystemState::new(world);
    let (filter, library, mut gizmos, agents, transforms) = state.get_mut(world);
    for (entity, agent, instance, blackboard, render, global_transform) in &agents {
        if let Some(filter) = filter.as_ref() {
            if filter.entity.is_some_and(|selected| selected != entity) {
                continue;
            }
            if filter
                .definition
                .is_some_and(|definition| definition != agent.definition)
            {
                continue;
            }
            if let Some(tag) = filter.tag.as_ref() {
                let matches_tag = library.get(agent.definition).is_some_and(|definition| {
                    instance.active_path.iter().any(|node| {
                        definition.node(*node).is_some_and(|node_definition| {
                            node_definition
                                .tags
                                .iter()
                                .any(|candidate| candidate == tag)
                        })
                    })
                });
                if !matches_tag {
                    continue;
                }
            }
        }
        let Some(global_transform) = global_transform else {
            continue;
        };
        let origin = global_transform.translation();
        let color = match instance.status {
            BehaviorTreeRunState::Running => Color::srgb(0.12, 0.87, 0.55),
            BehaviorTreeRunState::Success => Color::srgb(0.18, 0.62, 0.96),
            BehaviorTreeRunState::Failure => Color::srgb(0.93, 0.34, 0.26),
            BehaviorTreeRunState::Idle | BehaviorTreeRunState::Deactivated => {
                Color::srgb(0.55, 0.55, 0.55)
            }
        };
        gizmos.circle(origin + Vec3::Y * 0.05, render.ring_radius, color);
        let mut height = 0.2;
        for _ in &instance.active_path {
            gizmos.line(
                origin + Vec3::Y * height,
                origin + Vec3::Y * (height + render.vertical_spacing),
                color,
            );
            height += render.vertical_spacing;
        }
        if let Some(target_key) = render.target_entity_key
            && let Some(target) = blackboard.get_entity(target_key)
            && let Ok(target_transform) = transforms.get(target)
        {
            gizmos.line(
                origin + Vec3::Y * 0.4,
                target_transform.translation() + Vec3::Y * 0.4,
                Color::srgb(1.0, 0.85, 0.35),
            );
        }
    }
    state.apply(world);
}

fn ensure_instance(world: &mut World, entity: Entity) {
    let Some(agent) = world.get::<BehaviorTreeAgent>(entity).cloned() else {
        return;
    };
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed_secs);
    let Some(definition) = world
        .resource::<BehaviorTreeLibrary>()
        .get(agent.definition)
        .cloned()
    else {
        return;
    };

    let needs_new_instance = world
        .get::<BehaviorTreeInstance>(entity)
        .is_none_or(|instance| instance.definition != agent.definition);

    if needs_new_instance {
        let mut instance = BehaviorTreeInstance::new(
            agent.definition,
            definition.nodes.len(),
            agent.config.trace_capacity,
        );
        if let TickMode::Interval {
            seconds,
            phase_offset,
        } = agent.config.tick_mode
        {
            let seconds = seconds.max(0.001);
            let initial_offset = phase_offset.rem_euclid(seconds);
            if initial_offset > 0.0 {
                instance.wake_requested = false;
                instance.wake_reason.clear();
                instance.next_tick_at = now + initial_offset;
            }
        }
        world.entity_mut(entity).insert(instance);
    }

    if let Some(mut blackboard) = world.get_mut::<BehaviorTreeBlackboard>(entity) {
        let schema_changed = blackboard.schema != definition.blackboard_schema;
        if schema_changed || needs_new_instance {
            let preserve = agent.config.preserve_blackboard_on_definition_change;
            blackboard.resize_to_schema(&definition.blackboard_schema, preserve);
        }
    } else {
        world
            .entity_mut(entity)
            .insert(BehaviorTreeBlackboard::from_schema(
                &definition.blackboard_schema,
            ));
    }
}

#[cfg(test)]
#[path = "systems_tests.rs"]
mod tests;
