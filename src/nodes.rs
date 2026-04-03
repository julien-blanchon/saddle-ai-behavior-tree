use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::blackboard::{BlackboardCondition, BlackboardKeyId};
use crate::handlers::{ActionKey, ConditionKey, ServiceKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum AbortPolicy {
    None,
    SelfOnly,
    LowerPriority,
    Both,
}

impl AbortPolicy {
    pub fn monitors_self(self) -> bool {
        matches!(self, Self::SelfOnly | Self::Both)
    }

    pub fn monitors_lower_priority(self) -> bool {
        matches!(self, Self::LowerPriority | Self::Both)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SequenceKind {
    Sequence,
    SequenceWithMemory,
    ReactiveSequence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SelectorKind {
    Selector,
    SelectorWithMemory,
    ReactiveSelector { abort_policy: AbortPolicy },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum ParallelThreshold {
    Any,
    All,
    AtLeast(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct ParallelPolicy {
    pub success: ParallelThreshold,
    pub failure: ParallelThreshold,
    pub abort_running_siblings: bool,
}

impl ParallelPolicy {
    pub const fn all_success_any_failure() -> Self {
        Self {
            success: ParallelThreshold::All,
            failure: ParallelThreshold::Any,
            abort_running_siblings: true,
        }
    }

    pub const fn any_success_all_failure() -> Self {
        Self {
            success: ParallelThreshold::Any,
            failure: ParallelThreshold::All,
            abort_running_siblings: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub enum DecoratorKind {
    Inverter,
    Repeater {
        limit: Option<u32>,
    },
    Timeout {
        seconds: f32,
    },
    Cooldown {
        seconds: f32,
    },
    Retry {
        attempts: u32,
    },
    ForceSuccess,
    ForceFailure,
    Succeeder,
    UntilSuccess {
        limit: Option<u32>,
    },
    UntilFailure {
        limit: Option<u32>,
    },
    Limiter {
        limit: u32,
    },
    Guard {
        condition: ConditionKey,
        abort_policy: AbortPolicy,
        watch_keys: Vec<BlackboardKeyId>,
    },
    Delay {
        seconds: f32,
    },
    RunOnce {
        completed_status: BehaviorStatus,
    },
    BlackboardCondition {
        key: BlackboardKeyId,
        condition: BlackboardCondition,
        abort_policy: AbortPolicy,
    },
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub enum NodeKind {
    Sequence(SequenceKind),
    Selector(SelectorKind),
    Parallel(ParallelPolicy),
    Decorator(DecoratorKind),
    Action(ActionKey),
    Condition {
        key: ConditionKey,
        watch_keys: Vec<BlackboardKeyId>,
    },
}

#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ServiceBinding {
    pub name: String,
    pub key: ServiceKey,
    pub interval_seconds: f32,
    pub start_immediately: bool,
    pub wake_on_change: bool,
    pub watch_keys: Vec<BlackboardKeyId>,
}

impl ServiceBinding {
    pub fn new(name: impl Into<String>, key: impl Into<ServiceKey>, interval_seconds: f32) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            interval_seconds,
            start_immediately: true,
            wake_on_change: true,
            watch_keys: Vec::new(),
        }
    }

    pub fn with_watch_keys(mut self, keys: impl IntoIterator<Item = BlackboardKeyId>) -> Self {
        self.watch_keys = keys.into_iter().collect();
        self
    }
}
