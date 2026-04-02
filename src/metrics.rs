use bevy::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub struct BehaviorTreeMetrics {
    pub tick_count: u64,
    pub node_start_count: u64,
    pub node_finish_count: u64,
    pub abort_count: u64,
    pub service_run_count: u64,
    pub last_tick_micros: u64,
    pub node_execution_counts: Vec<u64>,
}

impl BehaviorTreeMetrics {
    pub fn for_node_count(node_count: usize) -> Self {
        Self {
            node_execution_counts: vec![0; node_count],
            ..default()
        }
    }
}
