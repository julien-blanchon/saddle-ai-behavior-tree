use saddle_bevy_e2e::action::Action;

use crate::LabStats;

pub(super) fn wait_for_service_ticks(min_ticks: usize, max_frames: u32) -> Action {
    Action::WaitUntil {
        label: format!("service ticks >= {min_ticks}"),
        condition: Box::new(move |world| world.resource::<LabStats>().service_ticks >= min_ticks),
        max_frames,
    }
}

pub(super) fn wait_for_aborts(min_aborts: usize, max_frames: u32) -> Action {
    Action::WaitUntil {
        label: format!("aborts >= {min_aborts}"),
        condition: Box::new(move |world| world.resource::<LabStats>().aborts >= min_aborts),
        max_frames,
    }
}

pub(super) fn wait_for_completions(min_completions: usize, max_frames: u32) -> Action {
    Action::WaitUntil {
        label: format!("completions >= {min_completions}"),
        condition: Box::new(move |world| {
            world.resource::<LabStats>().completions >= min_completions
        }),
        max_frames,
    }
}
