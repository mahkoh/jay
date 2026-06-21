use {
    crate::{
        configurable::ConfigureGroupsWork, state::State, transactions::TransactionsWork,
        utils::asyncevent::AsyncEvent,
    },
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
    let tsgs = &state.tree.serial_groups;
    let cgs = &state.tree.configure_groups;
    let ts = &state.tree.transactions;
    let mut cgsw = ConfigureGroupsWork::default();
    let mut tsw = TransactionsWork::default();
    loop {
        tsgs.scheduled.triggered().await;
        let serial = state.next_tree_serial();
        ts.commit(&state, &mut tsw, serial);
        cgs.run_scheduled(&mut cgsw, serial);
    }
}
