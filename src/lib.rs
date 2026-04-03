use bevy::ecs::intern::Interned;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::gizmos::prelude::GizmoConfigStore;
use bevy::prelude::*;

pub mod assets;
pub mod blackboard;
pub mod builder;
pub mod components;
pub mod debug;
pub mod definition;
pub mod execution;
pub mod handlers;
pub mod messages;
pub mod metrics;
pub mod nodes;
pub mod resources;
pub mod runtime;
pub mod services;
pub mod systems;

pub use assets::{
    BehaviorTreeDefinitionAsset, BehaviorTreeDefinitionAssetLoader,
    BehaviorTreeDefinitionAssetLoaderError,
};
pub use blackboard::{
    BehaviorTreeBlackboard, BlackboardChange, BlackboardCondition, BlackboardKeyDefinition,
    BlackboardKeyDirection, BlackboardKeyId, BlackboardSchema, BlackboardValue,
    BlackboardValueType,
};
pub use builder::{BehaviorTreeBuildError, BehaviorTreeBuilder, SubtreeRemap};
pub use components::BehaviorTreeAgent;
pub use debug::{
    BehaviorTreeDebugFilter, BehaviorTreeDebugGizmos, BehaviorTreeDebugRender, BehaviorTreeTrace,
    BehaviorTreeTraceEntry, TraceKind,
};
pub use definition::{BehaviorTreeDefinition, BehaviorTreeDefinitionId, NodeDefinition, NodeId};
pub use handlers::{
    ActionContext, ActionHandler, ActionTicket, ConditionContext, ConditionHandler, ServiceContext,
    ServiceHandler,
};
pub use messages::{
    ActionResolution, BlackboardValueChanged, BranchAborted, NodeFinished, NodeStarted,
    TreeCompleted, TreeResetRequested, TreeWakeRequested,
};
pub use metrics::BehaviorTreeMetrics;
pub use nodes::{
    AbortPolicy, BehaviorStatus, DecoratorKind, NodeKind, ParallelPolicy, ParallelThreshold,
    SelectorKind, SequenceKind, ServiceBinding,
};
pub use resources::{BehaviorTreeHandlers, BehaviorTreeLibrary};
pub use runtime::{
    BehaviorTreeConfig, BehaviorTreeInstance, BehaviorTreeRunState, NodeMemoryEntry,
    NodeRuntimeState, TickMode,
};

/// Public system ordering for the behavior-tree runtime.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BehaviorTreeSystems {
    Prepare,
    Evaluate,
    Apply,
    Cleanup,
    DebugRender,
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct NeverDeactivateSchedule;

/// Sandbox-friendly shared behavior-tree plugin.
pub struct BehaviorTreePlugin {
    pub activate_schedule: Interned<dyn ScheduleLabel>,
    pub deactivate_schedule: Interned<dyn ScheduleLabel>,
    pub update_schedule: Interned<dyn ScheduleLabel>,
}

impl BehaviorTreePlugin {
    pub fn new(
        activate_schedule: impl ScheduleLabel,
        deactivate_schedule: impl ScheduleLabel,
        update_schedule: impl ScheduleLabel,
    ) -> Self {
        Self {
            activate_schedule: activate_schedule.intern(),
            deactivate_schedule: deactivate_schedule.intern(),
            update_schedule: update_schedule.intern(),
        }
    }

    /// Convenience constructor for apps where trees stay live for the app lifetime.
    pub fn always_on(update_schedule: impl ScheduleLabel) -> Self {
        Self::new(PostStartup, NeverDeactivateSchedule, update_schedule)
    }
}

impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BehaviorTreeLibrary>()
            .init_resource::<BehaviorTreeHandlers>()
            .init_resource::<resources::ControlInbox>()
            .init_resource::<resources::RuntimeMessageBuffer>()
            .init_asset::<BehaviorTreeDefinitionAsset>()
            .register_asset_loader(BehaviorTreeDefinitionAssetLoader)
            .add_message::<TreeCompleted>()
            .add_message::<NodeStarted>()
            .add_message::<NodeFinished>()
            .add_message::<BranchAborted>()
            .add_message::<TreeWakeRequested>()
            .add_message::<TreeResetRequested>()
            .add_message::<ActionResolution>()
            .add_message::<BlackboardValueChanged>()
            .register_type::<AbortPolicy>()
            .register_type::<ActionResolution>()
            .register_type::<BehaviorStatus>()
            .register_type::<BehaviorTreeAgent>()
            .register_type::<BehaviorTreeBlackboard>()
            .register_type::<BehaviorTreeConfig>()
            .register_type::<BehaviorTreeDefinitionAsset>()
            .register_type::<BehaviorTreeDebugFilter>()
            .register_type::<BehaviorTreeDebugRender>()
            .register_type::<BehaviorTreeDefinition>()
            .register_type::<BehaviorTreeDefinitionId>()
            .register_type::<BehaviorTreeInstance>()
            .register_type::<BehaviorTreeLibrary>()
            .register_type::<BehaviorTreeMetrics>()
            .register_type::<BehaviorTreeRunState>()
            .register_type::<BehaviorTreeTrace>()
            .register_type::<BehaviorTreeTraceEntry>()
            .register_type::<BlackboardChange>()
            .register_type::<BlackboardCondition>()
            .register_type::<BlackboardKeyDefinition>()
            .register_type::<BlackboardKeyDirection>()
            .register_type::<BlackboardKeyId>()
            .register_type::<BlackboardSchema>()
            .register_type::<BlackboardValue>()
            .register_type::<BlackboardValueChanged>()
            .register_type::<BlackboardValueType>()
            .register_type::<BranchAborted>()
            .register_type::<DecoratorKind>()
            .register_type::<NodeDefinition>()
            .register_type::<NodeFinished>()
            .register_type::<NodeId>()
            .register_type::<NodeKind>()
            .register_type::<NodeMemoryEntry>()
            .register_type::<NodeRuntimeState>()
            .register_type::<NodeStarted>()
            .register_type::<ParallelPolicy>()
            .register_type::<ParallelThreshold>()
            .register_type::<SelectorKind>()
            .register_type::<SequenceKind>()
            .register_type::<ServiceBinding>()
            .register_type::<TickMode>()
            .register_type::<TraceKind>()
            .register_type::<TreeCompleted>()
            .register_type::<TreeResetRequested>()
            .register_type::<TreeWakeRequested>();

        app.add_systems(self.activate_schedule, systems::activate_agents);
        app.add_systems(self.deactivate_schedule, systems::deactivate_agents);
        app.add_systems(
            self.update_schedule,
            (
                (systems::ingest_control_messages, systems::prepare_agents)
                    .chain()
                    .in_set(BehaviorTreeSystems::Prepare),
                systems::evaluate_agents.in_set(BehaviorTreeSystems::Evaluate),
                systems::flush_runtime_messages.in_set(BehaviorTreeSystems::Apply),
                systems::cleanup_agents.in_set(BehaviorTreeSystems::Cleanup),
                systems::debug_render
                    .run_if(resource_exists::<GizmoConfigStore>)
                    .in_set(BehaviorTreeSystems::DebugRender),
            )
                .chain(),
        );
    }
}
