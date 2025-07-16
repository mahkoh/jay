use {
    crate::{
        backend::CONCAP_CONNECTOR,
        ifs::head_management::{HeadCommon, HeadState},
        state::ConnectorData,
        wire::{
            jay_head_ext_connector_info_v1::{
                Active, Connected, Disconnected, Inactive, JayHeadExtConnectorInfoV1RequestHandler,
            },
            jay_head_manager_ext_connector_info_v1::JayHeadManagerExtConnectorInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_connector_info_v1! {
    version = 1,
    filter = filter,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl MgrName {
    fn filter(&self, connector: &ConnectorData, _common: &Rc<HeadCommon>) -> bool {
        connector.connector.caps().contains(CONCAP_CONNECTOR)
    }
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_connected(shared);
        self.send_active(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.connected != tran.connected {
            self.send_connected(shared);
        }
        if shared.active != tran.active {
            self.send_active(shared);
        }
    }

    pub(in super::super) fn send_connected(&self, state: &HeadState) {
        if state.connected {
            self.client.event(Connected { self_id: self.id });
        } else {
            self.client.event(Disconnected { self_id: self.id });
        }
    }

    pub(in super::super) fn send_active(&self, state: &HeadState) {
        if state.active {
            self.client.event(Active { self_id: self.id });
        } else {
            self.client.event(Inactive { self_id: self.id });
        }
    }
}

impl JayHeadManagerExtConnectorInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtConnectorInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
