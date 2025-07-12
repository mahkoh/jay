use {
    crate::{
        ifs::head_management::HeadState,
        wire::{jay_head_ext_core_info_v1::*, jay_head_manager_ext_core_info_v1::*},
    },
    std::rc::Rc,
};

impl_core_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_name(shared);
        self.send_wl_output(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.wl_output != tran.wl_output {
            self.send_wl_output(shared);
        }
    }

    fn send_name(&self, state: &HeadState) {
        self.client.event(Name {
            self_id: self.id,
            name: Some(&**state.name),
        });
    }

    pub(in super::super) fn send_wl_output(&self, state: &HeadState) {
        match state.wl_output {
            None => {
                self.client.event(NoWlOutput { self_id: self.id });
            }
            Some(name) => {
                self.client.event(WlOutput {
                    self_id: self.id,
                    global_name: name.raw(),
                });
            }
        }
    }
}

impl JayHeadManagerExtCoreInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCoreInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
