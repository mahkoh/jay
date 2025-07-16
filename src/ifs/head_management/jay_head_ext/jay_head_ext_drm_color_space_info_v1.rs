use {
    crate::{
        backend::CONCAP_CONNECTOR,
        ifs::head_management::{HeadCommon, HeadState},
        state::ConnectorData,
        wire::{
            jay_head_ext_drm_color_space_info_v1::{
                Colorimetry, HdmiEotf, JayHeadExtDrmColorSpaceInfoV1RequestHandler,
            },
            jay_head_manager_ext_drm_color_space_info_v1::JayHeadManagerExtDrmColorSpaceInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_drm_color_space_info_v1! {
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
        self.send_state(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if (shared.color_space, shared.transfer_function)
            != (tran.color_space, tran.transfer_function)
        {
            self.send_state(shared);
        }
    }

    pub(in super::super) fn send_state(&self, state: &HeadState) {
        self.client.event(HdmiEotf {
            self_id: self.id,
            eotf: state.transfer_function.to_drm() as u32,
        });
        self.client.event(Colorimetry {
            self_id: self.id,
            colorimetry: state.color_space.to_drm() as u32,
        });
    }
}

impl JayHeadManagerExtDrmColorSpaceInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtDrmColorSpaceInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
