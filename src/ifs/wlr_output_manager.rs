use {
    crate::{
        ifs::wlr_output_manager::zwlr_output_head_v1::WlrOutputHeadIds,
        state::{OutputData, State},
        utils::{copyhashmap::CopyHashMap, queue::AsyncQueue},
    },
    std::rc::Rc,
    zwlr_output_manager_v1::{WlrOutputManagerId, WlrOutputManagerIds, ZwlrOutputManagerV1},
};

pub mod zwlr_output_configuration_head;
pub mod zwlr_output_configuration_v1;
pub mod zwlr_output_head_v1;
pub mod zwlr_output_manager_v1;
pub mod zwlr_output_mode_v1;

#[derive(Default)]
pub struct WlrOutputManagerState {
    queue: AsyncQueue<Rc<ZwlrOutputManagerV1>>,
    ids: WlrOutputManagerIds,
    head_ids: WlrOutputHeadIds,
    managers: CopyHashMap<WlrOutputManagerId, Rc<ZwlrOutputManagerV1>>,
}

impl WlrOutputManagerState {
    pub fn clear(&self) {
        self.managers.clear();
        self.queue.clear();
    }

    pub fn announce_head(&self, on: &Rc<OutputData>) {
        for manager in self.managers.lock().values() {
            manager.announce_head(on);
        }
    }
}

pub async fn wlr_output_manager_done(state: Rc<State>) {
    loop {
        let manager = state.wlr_output_managers.queue.pop().await;
        if manager.destroyed.get() {
            continue;
        }
        manager.done_scheduled.set(false);
        manager.send_done();
    }
}
