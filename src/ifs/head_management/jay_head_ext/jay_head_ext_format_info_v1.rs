use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_format_info_v1::{Format, JayHeadExtFormatInfoV1RequestHandler},
            jay_head_manager_ext_format_info_v1::JayHeadManagerExtFormatInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_format_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_format(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.format != tran.format {
            self.send_format(shared);
        }
    }

    pub(in super::super) fn send_format(&self, state: &HeadState) {
        self.client.event(Format {
            self_id: self.id,
            format: state.format.drm,
        });
    }
}

impl JayHeadManagerExtFormatInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtFormatInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
