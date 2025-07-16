use {
    crate::{
        ifs::head_management::{HeadOp, HeadState},
        scale::Scale,
        wire::{
            jay_head_ext_compositor_space_scaler_v1::{
                JayHeadExtCompositorSpaceScalerV1RequestHandler, Range, SetScale,
            },
            jay_head_manager_ext_compositor_space_scaler_v1::JayHeadManagerExtCompositorSpaceScalerV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_compositor_space_scaler_v1! {
    version = 1,
    after_announce = after_announce,
}

const MIN_SCALE: Scale = Scale::from_wl(60);
const MAX_SCALE: Scale = Scale::from_int(16);

impl HeadName {
    fn after_announce(&self, _shared: &HeadState) {
        self.send_range(MIN_SCALE, MAX_SCALE);
    }

    fn send_range(&self, min: Scale, max: Scale) {
        self.client.event(Range {
            self_id: self.id,
            min: min.to_wl(),
            max: max.to_wl(),
        });
    }
}

impl JayHeadManagerExtCompositorSpaceScalerV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCompositorSpaceScalerV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        let scale = Scale::from_wl(req.scale);
        if scale < MIN_SCALE || scale > MAX_SCALE {
            return Err(JayHeadExtCompositorSpaceScalerV1Error::ScaleOutOfBounds);
        }
        self.common.push_op(HeadOp::SetScale(scale))?;
        Ok(())
    }
}

error! {
    #[error("The scale is out of bounds")]
    ScaleOutOfBounds,
}
