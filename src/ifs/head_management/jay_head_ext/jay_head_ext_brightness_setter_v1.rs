use {
    crate::{
        ifs::head_management::HeadOp,
        wire::{
            jay_head_ext_brightness_setter_v1::{
                JayHeadExtBrightnessSetterV1RequestHandler, SetBrightness, UnsetBrightness,
            },
            jay_head_manager_ext_brightness_setter_v1::JayHeadManagerExtBrightnessSetterV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_brightness_setter_v1! {
    version = 1,
}

impl JayHeadManagerExtBrightnessSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtBrightnessSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn unset_brightness(&self, _req: UnsetBrightness, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.push_op(HeadOp::SetBrightness(None))?;
        Ok(())
    }

    fn set_brightness(&self, req: SetBrightness, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common
            .push_op(HeadOp::SetBrightness(Some(f32::from_bits(req.lux) as f64)))?;
        Ok(())
    }
}

error!();
