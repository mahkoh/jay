use {
    crate::{
        ifs::head_management::{HeadOp, HeadState},
        wire::{
            jay_head_ext_jay_vrr_mode_setter_v1::{
                JayHeadExtJayVrrModeSetterV1RequestHandler, SetMode, SupportedMode,
            },
            jay_head_manager_ext_jay_vrr_mode_setter_v1::JayHeadManagerExtJayVrrModeSetterV1RequestHandler,
        },
    },
    jay_config::video::VrrMode,
    std::rc::Rc,
};

impl_jay_vrr_mode_setter_v1! {
    version = 1,
    after_announce = after_announce,
}

impl HeadName {
    fn after_announce(&self, _shared: &HeadState) {
        self.send_supported_mode(VrrMode::NEVER);
        self.send_supported_mode(VrrMode::ALWAYS);
        self.send_supported_mode(VrrMode::VARIANT_1);
        self.send_supported_mode(VrrMode::VARIANT_2);
        self.send_supported_mode(VrrMode::VARIANT_3);
    }

    pub(in super::super) fn send_supported_mode(&self, mode: VrrMode) {
        self.client.event(SupportedMode {
            self_id: self.id,
            mode: mode.0,
        });
    }
}

impl JayHeadManagerExtJayVrrModeSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtJayVrrModeSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.mode > VrrMode::VARIANT_3.0 {
            return Err(ErrorName::UnknownMode(req.mode));
        }
        self.common.push_op(HeadOp::SetVrrMode(VrrMode(req.mode)))?;
        Ok(())
    }
}

error! {
    #[error("Unknown VRR mode {0}")]
    UnknownMode(u32),
}
