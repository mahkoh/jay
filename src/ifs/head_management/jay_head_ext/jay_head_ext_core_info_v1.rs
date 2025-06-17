use {
    crate::{
        client::ClientError,
        globals::GlobalName,
        ifs::head_management::HeadCommonError,
        state::ConnectorData,
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
    fn after_announce(&self, connector: &ConnectorData) {
        self.send_name(Some(&connector.name));
        if let Some(output) = self.client.state.outputs.get(&connector.id) {
            if let Some(node) = &output.node {
                self.send_wl_output(node.global.name);
            }
        }
    }

    pub fn send_name(&self, name: Option<&str>) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    pub fn send_wl_output(&self, name: GlobalName) {
        self.client.event(WlOutput {
            self_id: self.id,
            global_name: name.raw(),
        });
    }

    pub fn send_no_wl_output(&self) {
        self.client.event(NoWlOutput { self_id: self.id });
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
