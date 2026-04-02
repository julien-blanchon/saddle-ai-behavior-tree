use bevy::prelude::*;

use crate::blackboard::BlackboardKeyId;
use crate::definition::{BehaviorTreeDefinitionId, NodeId};
use crate::nodes::BehaviorStatus;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct BehaviorTreeDebugGizmos;

#[derive(Component, Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeDebugRender {
    pub ring_radius: f32,
    pub vertical_spacing: f32,
    pub target_entity_key: Option<BlackboardKeyId>,
}

impl Default for BehaviorTreeDebugRender {
    fn default() -> Self {
        Self {
            ring_radius: 0.8,
            vertical_spacing: 0.18,
            target_entity_key: None,
        }
    }
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Reflect)]
pub struct BehaviorTreeDebugFilter {
    pub entity: Option<Entity>,
    pub definition: Option<BehaviorTreeDefinitionId>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub struct BehaviorTreeTrace {
    pub capacity: usize,
    pub entries: Vec<BehaviorTreeTraceEntry>,
}

impl BehaviorTreeTrace {
    pub fn push(&mut self, entry: BehaviorTreeTraceEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.capacity {
            let overflow = self.entries.len() - self.capacity;
            self.entries.drain(0..overflow);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeTraceEntry {
    pub frame: u64,
    pub node: NodeId,
    pub kind: TraceKind,
    pub status: Option<BehaviorStatus>,
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum TraceKind {
    Started,
    Finished,
    Aborted,
    Service,
    Wake,
    BlackboardChanged,
}
