use {
    crate::{
        ifs::head_management::HeadOp,
        wire::{
            jay_head_ext_non_desktop_override_v1::{
                DisableOverride, JayHeadExtNonDesktopOverrideV1RequestHandler, OverrideDesktop,
                OverrideNonDesktop,
            },
            jay_head_manager_ext_non_desktop_override_v1::JayHeadManagerExtNonDesktopOverrideV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_non_desktop_override_v1! {
    version = 1,
}

impl JayHeadManagerExtNonDesktopOverrideV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtNonDesktopOverrideV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn disable_override(&self, _req: DisableOverride, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.push_op(HeadOp::SetNonDesktopOverride(None))?;
        Ok(())
    }

    fn override_desktop(&self, _req: OverrideDesktop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common
            .push_op(HeadOp::SetNonDesktopOverride(Some(false)))?;
        Ok(())
    }

    fn override_non_desktop(
        &self,
        _req: OverrideNonDesktop,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.common
            .push_op(HeadOp::SetNonDesktopOverride(Some(true)))?;
        Ok(())
    }
}

error!();
