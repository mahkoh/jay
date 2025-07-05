use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadOp},
        wire::{
            jay_head_ext_connector_settings_v1::{
                Disable, Enable, JayHeadExtConnectorSettingsV1RequestHandler,
            },
            jay_head_manager_ext_connector_settings_v1::JayHeadManagerExtConnectorSettingsV1RequestHandler,
        },
    },
    std::rc::Rc,
};

ext! {
    snake = connector_settings_v1,
    camel = ConnectorSettingsV1,
    version = 1,
}

impl JayHeadManagerExtConnectorSettingsV1RequestHandler for JayHeadManagerExtConnectorSettingsV1 {
    type Error = JayHeadExtConnectorSettingsV1Error;

    ext_common_req!(connector_settings_v1);
}

impl JayHeadExtConnectorSettingsV1RequestHandler for JayHeadExtConnectorSettingsV1 {
    type Error = JayHeadExtConnectorSettingsV1Error;

    head_common_req!(connector_settings_v1);

    fn enable(&self, _req: Enable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        self.common
            .pending
            .borrow_mut()
            .push(HeadOp::SetConnectorEnabled(true));
        Ok(())
    }

    fn disable(&self, _req: Disable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        self.common
            .pending
            .borrow_mut()
            .push(HeadOp::SetConnectorEnabled(false));
        Ok(())
    }
}

error! {
    ConnectorSettingsV1
}
