use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_tearing_state_v1::{
                Active, Disabled, Enabled, Inactive, JayHeadExtTearingStateV1RequestHandler,
            },
            jay_head_manager_ext_tearing_state_v1::JayHeadManagerExtTearingStateV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_tearing_state_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_enabled(shared);
        self.send_active(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.tearing_enabled != tran.tearing_enabled {
            self.send_enabled(shared);
        }
        if shared.tearing_active != tran.tearing_active {
            self.send_active(shared);
        }
    }

    pub(in super::super) fn send_enabled(&self, state: &HeadState) {
        if state.tearing_enabled {
            self.client.event(Enabled { self_id: self.id });
        } else {
            self.client.event(Disabled { self_id: self.id });
        }
    }

    pub(in super::super) fn send_active(&self, state: &HeadState) {
        if state.tearing_active {
            self.client.event(Active { self_id: self.id });
        } else {
            self.client.event(Inactive { self_id: self.id });
        }
    }
}

impl JayHeadManagerExtTearingStateV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtTearingStateV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
