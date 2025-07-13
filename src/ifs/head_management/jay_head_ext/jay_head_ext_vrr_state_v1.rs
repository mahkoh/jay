use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_vrr_state_v1::{
                Capable, Enabled, JayHeadExtVrrStateV1RequestHandler, Reset,
            },
            jay_head_manager_ext_vrr_state_v1::JayHeadManagerExtVrrStateV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_vrr_state_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_state(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        let shared_capable = shared.monitor_info.as_ref().map(|m| m.vrr_capable);
        let tran_capable = tran.monitor_info.as_ref().map(|m| m.vrr_capable);
        if (shared.vrr, shared_capable) != (tran.vrr, tran_capable) {
            self.send_state(shared);
        }
    }

    pub(in super::super) fn send_state(&self, state: &HeadState) {
        self.client.event(Reset { self_id: self.id });
        if let Some(mi) = &state.monitor_info
            && mi.vrr_capable
        {
            self.client.event(Capable { self_id: self.id });
        }
        if state.vrr {
            self.client.event(Enabled { self_id: self.id });
        }
    }
}

impl JayHeadManagerExtVrrStateV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtVrrStateV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
