use std::sync::Arc;

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::blackboard::{BehaviorTreeBlackboard, BlackboardKeyId, BlackboardValue};
use crate::definition::{BehaviorTreeDefinition, NodeId};
use crate::runtime::{NodeMemoryEntry, NodeRuntimeState};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ActionKey(pub String);

impl From<&str> for ActionKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ActionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ConditionKey(pub String);

impl From<&str> for ConditionKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ConditionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ServiceKey(pub String);

impl From<&str> for ServiceKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ServiceKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ActionTicket(pub u64);

pub struct ActionHandler {
    pub on_start:
        Arc<dyn for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus + Send + Sync>,
    pub on_tick:
        Arc<dyn for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus + Send + Sync>,
    pub on_abort: Option<Arc<dyn for<'w> Fn(&mut ActionContext<'w>) + Send + Sync>>,
}

impl Clone for ActionHandler {
    fn clone(&self) -> Self {
        Self {
            on_start: Arc::clone(&self.on_start),
            on_tick: Arc::clone(&self.on_tick),
            on_abort: self.on_abort.as_ref().map(Arc::clone),
        }
    }
}

impl ActionHandler {
    pub fn instant(
        handler: impl for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let handler: Arc<
            dyn for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus + Send + Sync,
        > = Arc::new(handler);
        Self {
            on_start: Arc::clone(&handler),
            on_tick: handler,
            on_abort: None,
        }
    }

    pub fn stateful(
        on_start: impl for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus
        + Send
        + Sync
        + 'static,
        on_tick: impl for<'w> Fn(&mut ActionContext<'w>) -> crate::nodes::BehaviorStatus
        + Send
        + Sync
        + 'static,
        on_abort: impl for<'w> Fn(&mut ActionContext<'w>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_start: Arc::new(on_start),
            on_tick: Arc::new(on_tick),
            on_abort: Some(Arc::new(on_abort)),
        }
    }
}

pub struct ConditionHandler {
    pub evaluate: Arc<dyn for<'w> Fn(&mut ConditionContext<'w>) -> bool + Send + Sync>,
}

impl Clone for ConditionHandler {
    fn clone(&self) -> Self {
        Self {
            evaluate: Arc::clone(&self.evaluate),
        }
    }
}

impl ConditionHandler {
    pub fn new(
        evaluate: impl for<'w> Fn(&mut ConditionContext<'w>) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            evaluate: Arc::new(evaluate),
        }
    }
}

pub struct ServiceHandler {
    pub tick: Arc<dyn for<'w> Fn(&mut ServiceContext<'w>) + Send + Sync>,
}

impl Clone for ServiceHandler {
    fn clone(&self) -> Self {
        Self {
            tick: Arc::clone(&self.tick),
        }
    }
}

impl ServiceHandler {
    pub fn new(tick: impl for<'w> Fn(&mut ServiceContext<'w>) + Send + Sync + 'static) -> Self {
        Self {
            tick: Arc::new(tick),
        }
    }
}

pub struct ActionContext<'w> {
    pub world: &'w mut World,
    pub entity: Entity,
    pub definition: &'w BehaviorTreeDefinition,
    pub blackboard: &'w mut BehaviorTreeBlackboard,
    pub node_id: NodeId,
    pub node_state: &'w mut NodeRuntimeState,
    pub action_ticket_counter: &'w mut u64,
    pub wake_requested: &'w mut bool,
    pub wake_reason: &'w mut String,
}

impl<'w> ActionContext<'w> {
    pub fn request_async_ticket(&mut self) -> ActionTicket {
        if let Some(ticket) = self.node_state.async_ticket {
            return ticket;
        }
        *self.action_ticket_counter += 1;
        let ticket = ActionTicket(*self.action_ticket_counter);
        self.node_state.async_ticket = Some(ticket);
        ticket
    }

    pub fn take_async_resolution(&mut self) -> Option<crate::nodes::BehaviorStatus> {
        self.node_state.async_resolution.take()
    }

    pub fn node_memory(&self, key: &str) -> Option<&BlackboardValue> {
        self.node_state
            .node_memory
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    pub fn set_node_memory(&mut self, key: impl Into<String>, value: impl Into<BlackboardValue>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self
            .node_state
            .node_memory
            .iter_mut()
            .find(|entry| entry.key == key)
        {
            entry.value = value;
            return;
        }
        self.node_state
            .node_memory
            .push(NodeMemoryEntry { key, value });
    }

    pub fn clear_node_memory(&mut self, key: &str) {
        self.node_state.node_memory.retain(|entry| entry.key != key);
    }

    pub fn write_message<M: Message>(&mut self, message: M) {
        self.world.resource_mut::<Messages<M>>().write(message);
    }

    pub fn wake_tree(&mut self, reason: impl Into<String>) {
        *self.wake_requested = true;
        *self.wake_reason = reason.into();
    }
}

pub struct ConditionContext<'w> {
    pub world: &'w World,
    pub entity: Entity,
    pub definition: &'w BehaviorTreeDefinition,
    pub blackboard: &'w BehaviorTreeBlackboard,
    pub node_id: NodeId,
}

impl<'w> ConditionContext<'w> {
    pub fn key(&self, name: &str) -> Option<BlackboardKeyId> {
        self.definition.find_blackboard_key(name)
    }
}

pub struct ServiceContext<'w> {
    pub world: &'w mut World,
    pub entity: Entity,
    pub definition: &'w BehaviorTreeDefinition,
    pub blackboard: &'w mut BehaviorTreeBlackboard,
    pub node_id: NodeId,
    pub node_state: &'w mut NodeRuntimeState,
    pub wake_requested: &'w mut bool,
    pub wake_reason: &'w mut String,
}

impl<'w> ServiceContext<'w> {
    pub fn set_node_memory(&mut self, key: impl Into<String>, value: impl Into<BlackboardValue>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self
            .node_state
            .node_memory
            .iter_mut()
            .find(|entry| entry.key == key)
        {
            entry.value = value;
            return;
        }
        self.node_state
            .node_memory
            .push(NodeMemoryEntry { key, value });
    }

    pub fn write_message<M: Message>(&mut self, message: M) {
        self.world.resource_mut::<Messages<M>>().write(message);
    }

    pub fn wake_tree(&mut self, reason: impl Into<String>) {
        *self.wake_requested = true;
        *self.wake_reason = reason.into();
    }
}
