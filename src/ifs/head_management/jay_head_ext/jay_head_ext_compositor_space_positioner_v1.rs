use {
    crate::{
        compositor::MAX_EXTENTS,
        ifs::head_management::{HeadOp, HeadState},
        wire::{
            jay_head_ext_compositor_space_positioner_v1::{
                JayHeadExtCompositorSpacePositionerV1RequestHandler, Range, SetPosition,
            },
            jay_head_manager_ext_compositor_space_positioner_v1::JayHeadManagerExtCompositorSpacePositionerV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_compositor_space_positioner_v1! {
    version = 1,
    after_announce = after_announce,
}

impl HeadName {
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

impl JayHeadManagerExtCompositorSpacePositionerV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCompositorSpacePositionerV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        if req.x < 0 || req.x > MAX_EXTENTS || req.y < 0 || req.y > MAX_EXTENTS {
            return Err(JayHeadExtCompositorSpacePositionerV1Error::PositionOutOfBounds);
        }
        self.common.push_op(HeadOp::SetPosition(req.x, req.y))?;
        Ok(())
    }
}

error! {
    #[error("The position is out of bounds")]
    PositionOutOfBounds,
}
