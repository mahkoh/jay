use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_jay_vrr_mode_info_v1::{JayHeadExtJayVrrModeInfoV1RequestHandler, Mode},
            jay_head_manager_ext_jay_vrr_mode_info_v1::JayHeadManagerExtJayVrrModeInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_jay_vrr_mode_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_mode(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.vrr_mode != tran.vrr_mode {
            self.send_mode(shared);
        }
    }

    pub(in super::super) fn send_mode(&self, state: &HeadState) {
        self.client.event(Mode {
            self_id: self.id,
            mode: state.vrr_mode.0,
        });
    }
}

impl JayHeadManagerExtJayVrrModeInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtJayVrrModeInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
