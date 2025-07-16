use {
    crate::{
        format::formats,
        ifs::head_management::{HeadOp, HeadState},
        wire::{
            jay_head_ext_format_setter_v1::{
                JayHeadExtFormatSetterV1RequestHandler, Reset, SetFormat, SupportedFormat,
            },
            jay_head_manager_ext_format_setter_v1::JayHeadManagerExtFormatSetterV1RequestHandler,
        },
    },
    isnt::std_1::primitive::IsntSlice2Ext,
    std::rc::Rc,
};

impl_format_setter_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_supported_formats(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.supported_formats != tran.supported_formats {
            self.send_supported_formats(shared);
        }
    }

    pub(in super::super) fn send_supported_formats(&self, state: &HeadState) {
        self.client.event(Reset { self_id: self.id });
        for format in &*state.supported_formats {
            self.client.event(SupportedFormat {
                self_id: self.id,
                format: format.drm,
            });
        }
    }
}

impl JayHeadManagerExtFormatSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtFormatSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_format(&self, req: SetFormat, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(format) = formats().get(&req.format) else {
            return Err(ErrorName::UnknownFormat(req.format));
        };
        if self
            .common
            .transaction_state
            .borrow()
            .supported_formats
            .not_contains(format)
        {
            return Err(ErrorName::UnsupportedFormat(req.format));
        }
        self.common.push_op(HeadOp::SetFormat(format))?;
        Ok(())
    }
}

error! {
    #[error("Unknown format {0}")]
    UnknownFormat(u32),
    #[error("Unsupported format {0}")]
    UnsupportedFormat(u32),
}
