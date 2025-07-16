use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_mode_info_v1::{JayHeadExtModeInfoV1RequestHandler, Mode},
            jay_head_manager_ext_mode_info_v1::JayHeadManagerExtModeInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_mode_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_mode(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.mode != tran.mode {
            self.send_mode(shared);
        }
    }

    pub(in super::super) fn send_mode(&self, state: &HeadState) {
        self.client.event(Mode {
            self_id: self.id,
            width: state.mode.width,
            height: state.mode.height,
            refresh_mhz: state.mode.refresh_rate_millihz,
        })
    }
}

impl JayHeadManagerExtModeInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtModeInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
