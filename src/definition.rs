use bevy::prelude::*;

use crate::blackboard::{BlackboardKeyId, BlackboardSchema};
use crate::nodes::{NodeKind, ServiceBinding};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct BehaviorTreeDefinitionId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct NodeId(pub u16);

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct NodeDefinition {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
    pub services: Vec<ServiceBinding>,
    pub tags: Vec<String>,
    pub watch_keys: Vec<BlackboardKeyId>,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeDefinition {
    pub name: String,
    pub root: NodeId,
    pub nodes: Vec<NodeDefinition>,
    pub blackboard_schema: BlackboardSchema,
    pub watched_keys: Vec<BlackboardKeyId>,
}

impl BehaviorTreeDefinition {
    pub fn node(&self, id: NodeId) -> Option<&NodeDefinition> {
        self.nodes.get(id.0 as usize)
    }

    pub fn find_blackboard_key(&self, name: &str) -> Option<BlackboardKeyId> {
        self.blackboard_schema.find_key(name)
    }
}
