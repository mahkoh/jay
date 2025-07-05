use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadOp, HeadState},
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

ext! {
    snake = compositor_space_scaler_v1,
    camel = CompositorSpaceScalerV1,
    version = 1,
    after_announce = after_announce,
}

pub const MIN_SCALE: Scale = Scale::from_int(1);
pub const MAX_SCALE: Scale = Scale::from_int(16);

impl JayHeadExtCompositorSpaceScalerV1 {
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

impl JayHeadManagerExtCompositorSpaceScalerV1RequestHandler
    for JayHeadManagerExtCompositorSpaceScalerV1
{
    type Error = JayHeadExtCompositorSpaceScalerV1Error;

    ext_common_req!(compositor_space_scaler_v1);
}

impl JayHeadExtCompositorSpaceScalerV1RequestHandler for JayHeadExtCompositorSpaceScalerV1 {
    type Error = JayHeadExtCompositorSpaceScalerV1Error;

    head_common_req!(compositor_space_scaler_v1);

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common
            .push_op(HeadOp::SetScale(Scale::from_wl(req.scale)))?;
        Ok(())
    }
}

error! {
    CompositorSpaceScalerV1,
}
