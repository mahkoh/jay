use {
    crate::{
        client::ClientError,
        compositor::MAX_EXTENTS,
        ifs::head_management::{HeadCommonError, HeadOp, HeadState},
        wire::{
            jay_head_ext_compositor_space_positioner_v1::{
                JayHeadExtCompositorSpacePositionerV1RequestHandler, Range, SetPosition,
            },
            jay_head_manager_ext_compositor_space_positioner_v1::JayHeadManagerExtCompositorSpacePositionerV1RequestHandler,
        },
    },
    std::rc::Rc,
};

ext! {
    snake = compositor_space_positioner_v1,
    camel = CompositorSpacePositionerV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtCompositorSpacePositionerV1 {
    fn after_announce(&self, _shared: &HeadState) {
        self.send_range();
    }

    fn send_range(&self) {
        self.client.event(Range {
            self_id: self.id,
            x_min: 0,
            y_min: 0,
            x_max: MAX_EXTENTS,
            y_max: MAX_EXTENTS,
        })
    }
}

impl JayHeadManagerExtCompositorSpacePositionerV1RequestHandler
    for JayHeadManagerExtCompositorSpacePositionerV1
{
    type Error = JayHeadExtCompositorSpacePositionerV1Error;

    ext_common_req!(compositor_space_positioner_v1);
}

impl JayHeadExtCompositorSpacePositionerV1RequestHandler for JayHeadExtCompositorSpacePositionerV1 {
    type Error = JayHeadExtCompositorSpacePositionerV1Error;

    head_common_req!(compositor_space_positioner_v1);

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.push_op(HeadOp::SetPosition(req.x, req.y))?;
        Ok(())
    }
}

error! {
    CompositorSpacePositionerV1
}
