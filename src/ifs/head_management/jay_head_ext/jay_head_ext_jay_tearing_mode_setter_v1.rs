use {
    crate::{
        ifs::head_management::{HeadOp, HeadState},
        wire::{
            jay_head_ext_jay_tearing_mode_setter_v1::{
                JayHeadExtJayTearingModeSetterV1RequestHandler, SetMode, SupportedMode,
            },
            jay_head_manager_ext_jay_tearing_mode_setter_v1::JayHeadManagerExtJayTearingModeSetterV1RequestHandler,
        },
    },
    jay_config::video::TearingMode,
    std::rc::Rc,
};

impl_jay_tearing_mode_setter_v1! {
    version = 1,
    after_announce = after_announce,
}

impl HeadName {
    fn after_announce(&self, _shared: &HeadState) {
        self.send_supported_mode(TearingMode::NEVER);
        self.send_supported_mode(TearingMode::ALWAYS);
        self.send_supported_mode(TearingMode::VARIANT_1);
        self.send_supported_mode(TearingMode::VARIANT_2);
        self.send_supported_mode(TearingMode::VARIANT_3);
    }

    pub(in super::super) fn send_supported_mode(&self, mode: TearingMode) {
        self.client.event(SupportedMode {
            self_id: self.id,
            mode: mode.0,
        });
    }
}

impl JayHeadManagerExtJayTearingModeSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtJayTearingModeSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.mode > TearingMode::VARIANT_3.0 {
            return Err(ErrorName::UnknownMode(req.mode));
        }
        self.common
            .push_op(HeadOp::SetTearingMode(TearingMode(req.mode)))?;
        Ok(())
    }
}

error! {
    #[error("Unknown tearing mode {0}")]
    UnknownMode(u32),
}
