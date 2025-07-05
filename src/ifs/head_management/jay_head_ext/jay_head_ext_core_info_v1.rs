use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadState},
        wire::{jay_head_ext_core_info_v1::*, jay_head_manager_ext_core_info_v1::*},
    },
    std::rc::Rc,
};

ext! {
    snake = core_info_v1,
    camel = CoreInfoV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtCoreInfoV1 {
    fn after_announce(&self, shared: &HeadState) {
        self.send_name(shared);
        self.send_wl_output(shared);
    }

    pub fn send_name(&self, state: &HeadState) {
        self.client.event(Name {
            self_id: self.id,
            name: Some(&**state.name),
        });
    }

    pub fn send_wl_output(&self, state: &HeadState) {
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

impl JayHeadManagerExtCoreInfoV1RequestHandler for JayHeadManagerExtCoreInfoV1 {
    type Error = JayHeadExtCoreInfoV1Error;

    ext_common_req!(core_info_v1);
}

impl JayHeadExtCoreInfoV1RequestHandler for JayHeadExtCoreInfoV1 {
    type Error = JayHeadExtCoreInfoV1Error;

    head_common_req!(core_info_v1);
}

error! {
    CoreInfoV1
}
