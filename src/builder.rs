use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use crate::blackboard::{
    BlackboardCondition, BlackboardKeyDefinition, BlackboardKeyDirection, BlackboardKeyId,
    BlackboardSchema, BlackboardValue,
};
use crate::definition::{BehaviorTreeDefinition, NodeDefinition, NodeId};
use crate::handlers::{ActionKey, ConditionKey};
use crate::nodes::{
    AbortPolicy, DecoratorKind, NodeKind, ParallelPolicy, SelectorKind, SequenceKind,
    ServiceBinding,
};

#[derive(Clone, Debug, PartialEq)]
pub enum BehaviorTreeBuildError {
    MissingRoot,
    UnknownNode(NodeId),
    InvalidChildCount {
        node: NodeId,
        expected: &'static str,
        found: usize,
    },
    UnknownSubtreeKey(String),
    RemappedKeyTypeMismatch {
        subtree_key: String,
        subtree_type: crate::blackboard::BlackboardValueType,
        target_type: crate::blackboard::BlackboardValueType,
    },
    CycleDetected(NodeId),
    UnreachableNode(NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubtreeRemap {
    pub local_key: String,
    pub target_key: BlackboardKeyId,
}

impl SubtreeRemap {
    pub fn new(local_key: impl Into<String>, target_key: BlackboardKeyId) -> Self {
        Self {
            local_key: local_key.into(),
            target_key,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BehaviorTreeBuilder {
    name: String,
    root: Option<NodeId>,
    blackboard_schema: BlackboardSchema,
    nodes: Vec<NodeDefinition>,
}

impl BehaviorTreeBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            root: None,
            blackboard_schema: BlackboardSchema::default(),
            nodes: Vec::new(),
        }
    }

    pub fn blackboard_key(
        &mut self,
        name: impl Into<String>,
        value_type: crate::blackboard::BlackboardValueType,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<BlackboardValue>,
        description: impl Into<String>,
    ) -> BlackboardKeyId {
        let id = BlackboardKeyId(self.blackboard_schema.keys.len() as u16);
        self.blackboard_schema.keys.push(BlackboardKeyDefinition {
            id,
            name: name.into(),
            value_type,
            direction,
            required,
            default_value,
            description: description.into(),
        });
        id
    }

    pub fn bool_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<bool>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Bool,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn int_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<i32>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Int,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn float_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<f32>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Float,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn entity_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<Entity>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Entity,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn vec2_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<Vec2>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Vec2,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn vec3_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<Vec3>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Vec3,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn quat_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<Quat>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Quat,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn text_key(
        &mut self,
        name: impl Into<String>,
        direction: BlackboardKeyDirection,
        required: bool,
        default_value: Option<&str>,
    ) -> BlackboardKeyId {
        self.blackboard_key(
            name,
            crate::blackboard::BlackboardValueType::Text,
            direction,
            required,
            default_value.map(BlackboardValue::from),
            "",
        )
    }

    pub fn action(&mut self, name: impl Into<String>, handler: impl Into<ActionKey>) -> NodeId {
        self.push_node(NodeDefinition {
            id: NodeId(0),
            name: name.into(),
            path: String::new(),
            kind: NodeKind::Action(handler.into()),
            children: Vec::new(),
            services: Vec::new(),
            tags: Vec::new(),
            watch_keys: Vec::new(),
        })
    }

    pub fn condition(
        &mut self,
        name: impl Into<String>,
        handler: impl Into<ConditionKey>,
    ) -> NodeId {
        self.condition_with_watch_keys(name, handler, [])
    }

    pub fn condition_with_watch_keys(
        &mut self,
        name: impl Into<String>,
        handler: impl Into<ConditionKey>,
        watch_keys: impl IntoIterator<Item = BlackboardKeyId>,
    ) -> NodeId {
        let watch_keys: Vec<_> = watch_keys.into_iter().collect();
        self.push_node(NodeDefinition {
            id: NodeId(0),
            name: name.into(),
            path: String::new(),
            kind: NodeKind::Condition {
                key: handler.into(),
                watch_keys: watch_keys.clone(),
            },
            children: Vec::new(),
            services: Vec::new(),
            tags: Vec::new(),
            watch_keys,
        })
    }

    pub fn sequence(
        &mut self,
        name: impl Into<String>,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(name, NodeKind::Sequence(SequenceKind::Sequence), children)
    }

    pub fn sequence_with_memory(
        &mut self,
        name: impl Into<String>,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(
            name,
            NodeKind::Sequence(SequenceKind::SequenceWithMemory),
            children,
        )
    }

    pub fn reactive_sequence(
        &mut self,
        name: impl Into<String>,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(
            name,
            NodeKind::Sequence(SequenceKind::ReactiveSequence),
            children,
        )
    }

    pub fn selector(
        &mut self,
        name: impl Into<String>,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(name, NodeKind::Selector(SelectorKind::Selector), children)
    }

    pub fn selector_with_memory(
        &mut self,
        name: impl Into<String>,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(
            name,
            NodeKind::Selector(SelectorKind::SelectorWithMemory),
            children,
        )
    }

    pub fn reactive_selector(
        &mut self,
        name: impl Into<String>,
        abort_policy: AbortPolicy,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(
            name,
            NodeKind::Selector(SelectorKind::ReactiveSelector { abort_policy }),
            children,
        )
    }

    pub fn parallel(
        &mut self,
        name: impl Into<String>,
        policy: ParallelPolicy,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.composite(name, NodeKind::Parallel(policy), children)
    }

    pub fn inverter(&mut self, name: impl Into<String>, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Inverter, child)
    }

    pub fn repeater(
        &mut self,
        name: impl Into<String>,
        limit: Option<u32>,
        child: NodeId,
    ) -> NodeId {
        self.decorator(name, DecoratorKind::Repeater { limit }, child)
    }

    pub fn timeout(&mut self, name: impl Into<String>, seconds: f32, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Timeout { seconds }, child)
    }

    pub fn cooldown(&mut self, name: impl Into<String>, seconds: f32, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Cooldown { seconds }, child)
    }

    pub fn retry(&mut self, name: impl Into<String>, attempts: u32, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Retry { attempts }, child)
    }

    pub fn force_success(&mut self, name: impl Into<String>, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::ForceSuccess, child)
    }

    pub fn force_failure(&mut self, name: impl Into<String>, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::ForceFailure, child)
    }

    pub fn succeeder(&mut self, name: impl Into<String>, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Succeeder, child)
    }

    pub fn until_success(
        &mut self,
        name: impl Into<String>,
        limit: Option<u32>,
        child: NodeId,
    ) -> NodeId {
        self.decorator(name, DecoratorKind::UntilSuccess { limit }, child)
    }

    pub fn until_failure(
        &mut self,
        name: impl Into<String>,
        limit: Option<u32>,
        child: NodeId,
    ) -> NodeId {
        self.decorator(name, DecoratorKind::UntilFailure { limit }, child)
    }

    pub fn limiter(&mut self, name: impl Into<String>, limit: u32, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Limiter { limit }, child)
    }

    pub fn delay(&mut self, name: impl Into<String>, seconds: f32, child: NodeId) -> NodeId {
        self.decorator(name, DecoratorKind::Delay { seconds }, child)
    }

    pub fn run_once(
        &mut self,
        name: impl Into<String>,
        completed_status: crate::nodes::BehaviorStatus,
        child: NodeId,
    ) -> NodeId {
        self.decorator(name, DecoratorKind::RunOnce { completed_status }, child)
    }

    pub fn guard(
        &mut self,
        name: impl Into<String>,
        condition: impl Into<ConditionKey>,
        abort_policy: AbortPolicy,
        watch_keys: impl IntoIterator<Item = BlackboardKeyId>,
        child: NodeId,
    ) -> NodeId {
        self.decorator(
            name,
            DecoratorKind::Guard {
                condition: condition.into(),
                abort_policy,
                watch_keys: watch_keys.into_iter().collect(),
            },
            child,
        )
    }

    pub fn blackboard_condition(
        &mut self,
        name: impl Into<String>,
        key: BlackboardKeyId,
        condition: BlackboardCondition,
        abort_policy: AbortPolicy,
        child: NodeId,
    ) -> NodeId {
        self.decorator(
            name,
            DecoratorKind::BlackboardCondition {
                key,
                condition,
                abort_policy,
            },
            child,
        )
    }

    pub fn add_service(&mut self, node: NodeId, service: ServiceBinding) -> &mut Self {
        if let Some(node_definition) = self.nodes.get_mut(node.0 as usize) {
            node_definition.services.push(service);
        }
        self
    }

    pub fn add_tag(&mut self, node: NodeId, tag: impl Into<String>) -> &mut Self {
        if let Some(node_definition) = self.nodes.get_mut(node.0 as usize) {
            node_definition.tags.push(tag.into());
        }
        self
    }

    pub fn set_root(&mut self, node: NodeId) -> &mut Self {
        self.root = Some(node);
        self
    }

    pub fn inline_subtree(
        &mut self,
        name: impl Into<String>,
        subtree: &BehaviorTreeDefinition,
        remaps: impl IntoIterator<Item = SubtreeRemap>,
    ) -> Result<NodeId, BehaviorTreeBuildError> {
        let name = name.into();
        let remaps: BTreeMap<_, _> = remaps
            .into_iter()
            .map(|remap| (remap.local_key, remap.target_key))
            .collect();
        let mut key_map = BTreeMap::new();
        for subtree_key in &subtree.blackboard_schema.keys {
            if let Some(target_key) = remaps.get(&subtree_key.name) {
                let Some(target_definition) = self.blackboard_schema.key(*target_key) else {
                    return Err(BehaviorTreeBuildError::UnknownSubtreeKey(
                        subtree_key.name.clone(),
                    ));
                };
                if target_definition.value_type != subtree_key.value_type {
                    return Err(BehaviorTreeBuildError::RemappedKeyTypeMismatch {
                        subtree_key: subtree_key.name.clone(),
                        subtree_type: subtree_key.value_type,
                        target_type: target_definition.value_type,
                    });
                }
                key_map.insert(subtree_key.id, *target_key);
            } else {
                let new_key = self.blackboard_key(
                    format!("{name}.{}", subtree_key.name),
                    subtree_key.value_type,
                    BlackboardKeyDirection::Local,
                    subtree_key.required,
                    subtree_key.default_value.clone(),
                    subtree_key.description.clone(),
                );
                key_map.insert(subtree_key.id, new_key);
            }
        }

        let mut node_map = BTreeMap::new();
        for node in &subtree.nodes {
            let new_id = NodeId(self.nodes.len() as u16);
            node_map.insert(node.id, new_id);
            self.nodes.push(NodeDefinition {
                id: new_id,
                name: format!("{name}/{}", node.name),
                path: format!("{name}/{}", node.path),
                kind: remap_node_kind(&node.kind, &key_map),
                children: Vec::new(),
                services: remap_services(&node.services, &key_map),
                tags: node.tags.clone(),
                watch_keys: remap_watch_keys(&node.watch_keys, &key_map),
            });
        }
        for node in &subtree.nodes {
            let new_id = node_map[&node.id];
            self.nodes[new_id.0 as usize].children =
                node.children.iter().map(|child| node_map[child]).collect();
        }
        Ok(node_map[&subtree.root])
    }

    pub fn build(mut self) -> Result<BehaviorTreeDefinition, BehaviorTreeBuildError> {
        let Some(root) = self.root else {
            return Err(BehaviorTreeBuildError::MissingRoot);
        };
        for index in 0..self.nodes.len() {
            let node_id = NodeId(index as u16);
            let children = self.nodes[index].children.clone();
            {
                let node = &mut self.nodes[index];
                node.id = node_id;
                if node.path.is_empty() {
                    node.path = node.name.clone();
                }
                validate_child_count(node)?;
            }
            for child in &children {
                if self.nodes.get(child.0 as usize).is_none() {
                    return Err(BehaviorTreeBuildError::UnknownNode(*child));
                }
            }
        }

        let mut color = vec![0u8; self.nodes.len()];
        visit_node(root, &self.nodes, &mut color)?;
        for (index, state) in color.into_iter().enumerate() {
            if state == 0 {
                return Err(BehaviorTreeBuildError::UnreachableNode(NodeId(
                    index as u16,
                )));
            }
        }

        let mut watched_keys = BTreeSet::new();
        for node in &self.nodes {
            for key in &node.watch_keys {
                watched_keys.insert(*key);
            }
            match &node.kind {
                NodeKind::Condition { watch_keys, .. } => {
                    for key in watch_keys {
                        watched_keys.insert(*key);
                    }
                }
                NodeKind::Decorator(DecoratorKind::Guard { watch_keys, .. }) => {
                    for key in watch_keys {
                        watched_keys.insert(*key);
                    }
                }
                NodeKind::Decorator(DecoratorKind::BlackboardCondition { key, .. }) => {
                    watched_keys.insert(*key);
                }
                _ => {}
            }
            for service in &node.services {
                for key in &service.watch_keys {
                    watched_keys.insert(*key);
                }
            }
        }

        Ok(BehaviorTreeDefinition {
            name: self.name,
            root,
            nodes: self.nodes,
            blackboard_schema: self.blackboard_schema,
            watched_keys: watched_keys.into_iter().collect(),
        })
    }

    fn composite(
        &mut self,
        name: impl Into<String>,
        kind: NodeKind,
        children: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        self.push_node(NodeDefinition {
            id: NodeId(0),
            name: name.into(),
            path: String::new(),
            kind,
            children: children.into_iter().collect(),
            services: Vec::new(),
            tags: Vec::new(),
            watch_keys: Vec::new(),
        })
    }

    fn decorator(
        &mut self,
        name: impl Into<String>,
        decorator: DecoratorKind,
        child: NodeId,
    ) -> NodeId {
        let mut watch_keys = Vec::new();
        match &decorator {
            DecoratorKind::Guard {
                watch_keys: keys, ..
            } => watch_keys = keys.clone(),
            DecoratorKind::BlackboardCondition { key, .. } => watch_keys.push(*key),
            _ => {}
        }
        self.push_node(NodeDefinition {
            id: NodeId(0),
            name: name.into(),
            path: String::new(),
            kind: NodeKind::Decorator(decorator),
            children: vec![child],
            services: Vec::new(),
            tags: Vec::new(),
            watch_keys,
        })
    }

    fn push_node(&mut self, mut node: NodeDefinition) -> NodeId {
        let id = NodeId(self.nodes.len() as u16);
        node.id = id;
        self.nodes.push(node);
        id
    }
}

fn remap_node_kind(
    kind: &NodeKind,
    key_map: &BTreeMap<BlackboardKeyId, BlackboardKeyId>,
) -> NodeKind {
    match kind {
        NodeKind::Sequence(kind) => NodeKind::Sequence(*kind),
        NodeKind::Selector(kind) => NodeKind::Selector(*kind),
        NodeKind::Parallel(policy) => NodeKind::Parallel(*policy),
        NodeKind::Action(key) => NodeKind::Action(key.clone()),
        NodeKind::Condition { key, watch_keys } => NodeKind::Condition {
            key: key.clone(),
            watch_keys: remap_watch_keys(watch_keys, key_map),
        },
        NodeKind::Decorator(decorator) => NodeKind::Decorator(match decorator {
            DecoratorKind::Inverter => DecoratorKind::Inverter,
            DecoratorKind::Repeater { limit } => DecoratorKind::Repeater { limit: *limit },
            DecoratorKind::Timeout { seconds } => DecoratorKind::Timeout { seconds: *seconds },
            DecoratorKind::Cooldown { seconds } => DecoratorKind::Cooldown { seconds: *seconds },
            DecoratorKind::Retry { attempts } => DecoratorKind::Retry {
                attempts: *attempts,
            },
            DecoratorKind::ForceSuccess => DecoratorKind::ForceSuccess,
            DecoratorKind::ForceFailure => DecoratorKind::ForceFailure,
            DecoratorKind::Succeeder => DecoratorKind::Succeeder,
            DecoratorKind::UntilSuccess { limit } => DecoratorKind::UntilSuccess { limit: *limit },
            DecoratorKind::UntilFailure { limit } => DecoratorKind::UntilFailure { limit: *limit },
            DecoratorKind::Limiter { limit } => DecoratorKind::Limiter { limit: *limit },
            DecoratorKind::Guard {
                condition,
                abort_policy,
                watch_keys,
            } => DecoratorKind::Guard {
                condition: condition.clone(),
                abort_policy: *abort_policy,
                watch_keys: remap_watch_keys(watch_keys, key_map),
            },
            DecoratorKind::Delay { seconds } => DecoratorKind::Delay { seconds: *seconds },
            DecoratorKind::RunOnce { completed_status } => DecoratorKind::RunOnce {
                completed_status: *completed_status,
            },
            DecoratorKind::BlackboardCondition {
                key,
                condition,
                abort_policy,
            } => DecoratorKind::BlackboardCondition {
                key: key_map[key],
                condition: condition.clone(),
                abort_policy: *abort_policy,
            },
        }),
    }
}

fn remap_services(
    services: &[ServiceBinding],
    key_map: &BTreeMap<BlackboardKeyId, BlackboardKeyId>,
) -> Vec<ServiceBinding> {
    services
        .iter()
        .map(|service| ServiceBinding {
            name: service.name.clone(),
            key: service.key.clone(),
            interval_seconds: service.interval_seconds,
            start_immediately: service.start_immediately,
            wake_on_change: service.wake_on_change,
            watch_keys: remap_watch_keys(&service.watch_keys, key_map),
        })
        .collect()
}

fn remap_watch_keys(
    watch_keys: &[BlackboardKeyId],
    key_map: &BTreeMap<BlackboardKeyId, BlackboardKeyId>,
) -> Vec<BlackboardKeyId> {
    watch_keys
        .iter()
        .map(|key| *key_map.get(key).unwrap_or(key))
        .collect()
}

fn validate_child_count(node: &NodeDefinition) -> Result<(), BehaviorTreeBuildError> {
    match node.kind {
        NodeKind::Action(_) | NodeKind::Condition { .. } if !node.children.is_empty() => {
            Err(BehaviorTreeBuildError::InvalidChildCount {
                node: node.id,
                expected: "leaf node with 0 children",
                found: node.children.len(),
            })
        }
        NodeKind::Decorator(_) if node.children.len() != 1 => {
            Err(BehaviorTreeBuildError::InvalidChildCount {
                node: node.id,
                expected: "decorator with exactly 1 child",
                found: node.children.len(),
            })
        }
        _ => Ok(()),
    }
}

fn visit_node(
    node: NodeId,
    nodes: &[NodeDefinition],
    color: &mut [u8],
) -> Result<(), BehaviorTreeBuildError> {
    let index = node.0 as usize;
    match color[index] {
        1 => return Err(BehaviorTreeBuildError::CycleDetected(node)),
        2 => return Ok(()),
        _ => {}
    }
    color[index] = 1;
    for child in &nodes[index].children {
        visit_node(*child, nodes, color)?;
    }
    color[index] = 2;
    Ok(())
}
