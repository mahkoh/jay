use {
    crate::{configurable::ConfigureGroupsWork, state::State, utils::asyncevent::AsyncEvent},
    std::rc::Rc,
};

#[derive(Default)]
pub struct TreeSerialGroups {
    scheduled: AsyncEvent,
}

impl TreeSerialGroups {
    pub fn trigger(&self) {
        self.scheduled.trigger();
    }
}

pub async fn handle_tree_serial_groups_scheduled(state: Rc<State>) {
    let tsgs = &state.tree_serial_groups;
    let cgs = &state.configure_groups;
    let mut cgsw = ConfigureGroupsWork::default();
    loop {
        tsgs.scheduled.triggered().await;
        let serial = state.next_tree_serial();
        cgs.run_scheduled(&mut cgsw, serial);
    }
}
