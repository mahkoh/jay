use std::rc::Rc;

pub mod zwlr_output_configuration_head;
pub mod zwlr_output_configuration_v1;
pub mod zwlr_output_head_v1;
pub mod zwlr_output_manager_v1;
pub mod zwlr_output_mode_v1;

use {
    crate::{
        state::State,
        tree::OutputNode,
        utils::{copyhashmap::CopyHashMap, opt::Opt, queue::AsyncQueue},
    },
    zwlr_output_configuration_v1::{
        OutputConfigurationId, OutputConfigurationIds, ZwlrOutputConfigurationV1,
    },
    zwlr_output_manager_v1::{OutputManagerId, OutputManagerIds, ZwlrOutputManagerV1},
};

#[derive(Default)]
pub struct OutputManagerState {
    queue: AsyncQueue<Rc<Opt<ZwlrOutputManagerV1>>>,
    ids: OutputManagerIds,
    managers: CopyHashMap<OutputManagerId, Rc<ZwlrOutputManagerV1>>,
    configiuration_ids: OutputConfigurationIds,
    configurations: CopyHashMap<OutputConfigurationId, Rc<ZwlrOutputConfigurationV1>>,
}

impl OutputManagerState {
    pub fn clear(&self) {
        self.managers.clear();
        self.configurations.clear();
        self.queue.clear();
    }

    pub fn announce_head(&self, on: &OutputNode) {
        for manager in self.managers.lock().values() {
            manager.announce_head(on);
        }
    }
}

pub async fn output_manager_done(state: Rc<State>) {
    loop {
        let manager = state.output_managers.queue.pop().await;
        if let Some(manager) = manager.get() {
            if manager.done_scheduled.get() {
                let serial = manager.serial.get() + 1;
                manager.send_done(serial);
            }
        }
    }
}
