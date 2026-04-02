use bevy::prelude::*;

use crate::blackboard::{BlackboardKeyId, BlackboardValue};
use crate::definition::{BehaviorTreeDefinitionId, NodeId};
use crate::handlers::ActionTicket;
use crate::nodes::BehaviorStatus;

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct TreeCompleted {
    pub entity: Entity,
    pub definition: BehaviorTreeDefinitionId,
    pub status: BehaviorStatus,
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct NodeStarted {
    pub entity: Entity,
    pub node: NodeId,
    pub path: String,
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct NodeFinished {
    pub entity: Entity,
    pub node: NodeId,
    pub path: String,
    pub status: BehaviorStatus,
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct BranchAborted {
    pub entity: Entity,
    pub node: NodeId,
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct TreeWakeRequested {
    pub entity: Entity,
    pub reason: String,
}

impl TreeWakeRequested {
    pub fn new(entity: Entity, reason: impl Into<String>) -> Self {
        Self {
            entity,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct TreeResetRequested {
    pub entity: Entity,
    pub reason: String,
}

impl TreeResetRequested {
    pub fn new(entity: Entity, reason: impl Into<String>) -> Self {
        Self {
            entity,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct ActionResolution {
    pub entity: Entity,
    pub ticket: ActionTicket,
    pub status: BehaviorStatus,
}

impl ActionResolution {
    pub fn new(entity: Entity, ticket: ActionTicket, status: BehaviorStatus) -> Self {
        Self {
            entity,
            ticket,
            status,
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq, Reflect)]
pub struct BlackboardValueChanged {
    pub entity: Entity,
    pub key: BlackboardKeyId,
    pub name: String,
    pub revision: u64,
    pub old_value: Option<BlackboardValue>,
    pub new_value: Option<BlackboardValue>,
}
