use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadState},
        wire::{
            jay_head_ext_connector_info_v1::{
                Connected, Disabled, Disconnected, Enabled, JayHeadExtConnectorInfoV1RequestHandler,
            },
            jay_head_manager_ext_connector_info_v1::JayHeadManagerExtConnectorInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

ext! {
    snake = connector_info_v1,
    camel = ConnectorInfoV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtConnectorInfoV1 {
    fn after_announce(&self, shared: &HeadState) {
        self.send_enabled(shared);
        self.send_connected(shared);
    }

    pub fn send_enabled(&self, shared: &HeadState) {
        if shared.connector_enabled {
            self.client.event(Enabled { self_id: self.id });
        } else {
            self.client.event(Disabled { self_id: self.id });
        }
    }

    pub fn send_connected(&self, shared: &HeadState) {
        if shared.connected {
            self.client.event(Connected { self_id: self.id });
        } else {
            self.client.event(Disconnected { self_id: self.id });
        }
    }
}

impl JayHeadManagerExtConnectorInfoV1RequestHandler for JayHeadManagerExtConnectorInfoV1 {
    type Error = JayHeadExtConnectorInfoV1Error;

    ext_common_req!(connector_info_v1);
}

impl JayHeadExtConnectorInfoV1RequestHandler for JayHeadExtConnectorInfoV1 {
    type Error = JayHeadExtConnectorInfoV1Error;

    head_common_req!(connector_info_v1);
}

error! {
    ConnectorInfoV1
}
