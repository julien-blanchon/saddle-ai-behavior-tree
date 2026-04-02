use bevy::prelude::*;

use crate::definition::BehaviorTreeDefinitionId;
use crate::runtime::BehaviorTreeConfig;

#[derive(Component, Clone, Debug, PartialEq, Reflect)]
pub struct BehaviorTreeAgent {
    pub definition: BehaviorTreeDefinitionId,
    pub config: BehaviorTreeConfig,
    pub enabled: bool,
}

impl BehaviorTreeAgent {
    pub fn new(definition: BehaviorTreeDefinitionId) -> Self {
        Self {
            definition,
            config: BehaviorTreeConfig::default(),
            enabled: true,
        }
    }

    pub fn with_config(mut self, config: BehaviorTreeConfig) -> Self {
        self.config = config;
        self
    }
}
