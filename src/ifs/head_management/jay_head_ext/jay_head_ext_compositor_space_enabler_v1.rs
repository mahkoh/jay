use {
    crate::{
        backend::CONCAP_CONNECTOR,
        ifs::head_management::{HeadCommon, HeadOp},
        state::ConnectorData,
        wire::{
            jay_head_ext_compositor_space_enabler_v1::{
                Disable, Enable, JayHeadExtCompositorSpaceEnablerV1RequestHandler,
            },
            jay_head_manager_ext_compositor_space_enabler_v1::JayHeadManagerExtCompositorSpaceEnablerV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_compositor_space_enabler_v1! {
    version = 1,
    filter = filter,
}

impl MgrName {
    fn filter(&self, connector: &ConnectorData, _common: &Rc<HeadCommon>) -> bool {
        connector.connector.caps().contains(CONCAP_CONNECTOR)
    }
}

impl JayHeadManagerExtCompositorSpaceEnablerV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCompositorSpaceEnablerV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn enable(&self, _req: Enable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.push_op(HeadOp::SetConnectorEnabled(true))?;
        Ok(())
    }

    fn disable(&self, _req: Disable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.push_op(HeadOp::SetConnectorEnabled(false))?;
        Ok(())
    }
}

error!();
