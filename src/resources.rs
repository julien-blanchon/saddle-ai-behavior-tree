use bevy::prelude::*;

use crate::definition::{BehaviorTreeDefinition, BehaviorTreeDefinitionId};
use crate::handlers::{
    ActionHandler, ActionKey, ConditionHandler, ConditionKey, ServiceHandler, ServiceKey,
};
use crate::messages::{
    BlackboardValueChanged, BranchAborted, NodeFinished, NodeStarted, TreeCompleted,
    TreeResetRequested, TreeWakeRequested,
};

#[derive(Resource, Clone, Debug, Default, PartialEq, Reflect)]
pub struct BehaviorTreeLibrary {
    pub definitions: Vec<BehaviorTreeDefinition>,
}

impl BehaviorTreeLibrary {
    pub fn register(
        &mut self,
        definition: BehaviorTreeDefinition,
    ) -> Result<BehaviorTreeDefinitionId, String> {
        let id = BehaviorTreeDefinitionId(self.definitions.len() as u16);
        self.definitions.push(definition);
        Ok(id)
    }

    pub fn get(&self, id: BehaviorTreeDefinitionId) -> Option<&BehaviorTreeDefinition> {
        self.definitions.get(id.0 as usize)
    }
}

#[derive(Resource, Default, Clone)]
pub struct BehaviorTreeHandlers {
    pub actions: Vec<(ActionKey, ActionHandler)>,
    pub conditions: Vec<(ConditionKey, ConditionHandler)>,
    pub services: Vec<(ServiceKey, ServiceHandler)>,
}

impl BehaviorTreeHandlers {
    pub fn register_action(&mut self, key: impl Into<ActionKey>, handler: ActionHandler) {
        let key = key.into();
        if let Some(existing) = self
            .actions
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            existing.1 = handler;
            return;
        }
        self.actions.push((key, handler));
    }

    pub fn register_condition(&mut self, key: impl Into<ConditionKey>, handler: ConditionHandler) {
        let key = key.into();
        if let Some(existing) = self
            .conditions
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            existing.1 = handler;
            return;
        }
        self.conditions.push((key, handler));
    }

    pub fn register_service(&mut self, key: impl Into<ServiceKey>, handler: ServiceHandler) {
        let key = key.into();
        if let Some(existing) = self
            .services
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            existing.1 = handler;
            return;
        }
        self.services.push((key, handler));
    }

    pub fn action(&self, key: &ActionKey) -> Option<&ActionHandler> {
        self.actions
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, handler)| handler)
    }

    pub fn condition(&self, key: &ConditionKey) -> Option<&ConditionHandler> {
        self.conditions
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, handler)| handler)
    }

    pub fn service(&self, key: &ServiceKey) -> Option<&ServiceHandler> {
        self.services
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, handler)| handler)
    }
}

#[derive(Resource, Default)]
pub(crate) struct ControlInbox {
    pub wake_requests: Vec<TreeWakeRequested>,
    pub reset_requests: Vec<TreeResetRequested>,
    pub action_resolutions: Vec<crate::messages::ActionResolution>,
}

#[derive(Resource, Default)]
pub(crate) struct RuntimeMessageBuffer {
    pub tree_completed: Vec<TreeCompleted>,
    pub node_started: Vec<NodeStarted>,
    pub node_finished: Vec<NodeFinished>,
    pub branch_aborted: Vec<BranchAborted>,
    pub blackboard_changed: Vec<BlackboardValueChanged>,
}
