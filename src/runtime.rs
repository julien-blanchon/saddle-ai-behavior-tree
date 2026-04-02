use bevy::prelude::*;

use crate::debug::BehaviorTreeTrace;
use crate::definition::{BehaviorTreeDefinitionId, NodeId};
use crate::handlers::ActionTicket;
use crate::metrics::BehaviorTreeMetrics;
use crate::nodes::BehaviorStatus;

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub enum TickMode {
    #[default]
    EveryFrame,
    Interval {
        seconds: f32,
        phase_offset: f32,
    },
    Manual,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeConfig {
    pub tick_mode: TickMode,
    pub restart_on_completion: bool,
    pub preserve_blackboard_on_definition_change: bool,
    pub emit_lifecycle_messages: bool,
    pub emit_blackboard_messages: bool,
    pub trace_capacity: usize,
}

impl Default for BehaviorTreeConfig {
    fn default() -> Self {
        Self {
            tick_mode: TickMode::EveryFrame,
            restart_on_completion: false,
            preserve_blackboard_on_definition_change: true,
            emit_lifecycle_messages: false,
            emit_blackboard_messages: false,
            trace_capacity: 64,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub enum BehaviorTreeRunState {
    #[default]
    Idle,
    Running,
    Success,
    Failure,
    Deactivated,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct NodeMemoryEntry {
    pub key: String,
    pub value: crate::blackboard::BlackboardValue,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct NodeRuntimeState {
    pub status: BehaviorTreeRunState,
    pub cursor: usize,
    pub entered_at: f32,
    pub finished_at: f32,
    pub cooldown_until: f32,
    pub delay_until: Option<f32>,
    pub counter: u32,
    pub limiter_used: u32,
    pub service_due_at: Vec<f32>,
    pub node_memory: Vec<NodeMemoryEntry>,
    pub async_ticket: Option<ActionTicket>,
    pub async_resolution: Option<BehaviorStatus>,
    pub last_result: Option<BehaviorStatus>,
    pub execution_count: u64,
}

impl Default for NodeRuntimeState {
    fn default() -> Self {
        Self {
            status: BehaviorTreeRunState::Idle,
            cursor: 0,
            entered_at: 0.0,
            finished_at: 0.0,
            cooldown_until: 0.0,
            delay_until: None,
            counter: 0,
            limiter_used: 0,
            service_due_at: Vec::new(),
            node_memory: Vec::new(),
            async_ticket: None,
            async_resolution: None,
            last_result: None,
            execution_count: 0,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeInstance {
    pub definition: BehaviorTreeDefinitionId,
    pub status: BehaviorTreeRunState,
    pub active_path: Vec<NodeId>,
    pub last_running_leaf: Option<NodeId>,
    pub last_abort_reason: String,
    pub wake_requested: bool,
    pub wake_reason: String,
    pub next_tick_at: f32,
    pub observed_blackboard_revision: u64,
    pub action_ticket_counter: u64,
    pub metrics: BehaviorTreeMetrics,
    pub trace: BehaviorTreeTrace,
    pub node_states: Vec<NodeRuntimeState>,
}

impl BehaviorTreeInstance {
    pub fn new(
        definition: BehaviorTreeDefinitionId,
        node_count: usize,
        trace_capacity: usize,
    ) -> Self {
        Self {
            definition,
            status: BehaviorTreeRunState::Idle,
            active_path: Vec::new(),
            last_running_leaf: None,
            last_abort_reason: String::new(),
            wake_requested: true,
            wake_reason: "initial activation".to_owned(),
            next_tick_at: 0.0,
            observed_blackboard_revision: 0,
            action_ticket_counter: 0,
            metrics: BehaviorTreeMetrics::for_node_count(node_count),
            trace: BehaviorTreeTrace {
                capacity: trace_capacity,
                entries: Vec::new(),
            },
            node_states: vec![NodeRuntimeState::default(); node_count],
        }
    }

    pub fn next_action_ticket(&mut self) -> ActionTicket {
        self.action_ticket_counter += 1;
        ActionTicket(self.action_ticket_counter)
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
