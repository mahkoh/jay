use {
    crate::{
        ifs::head_management::{HeadOp, HeadState},
        utils::transform_ext::TransformExt,
        wire::{
            jay_head_ext_compositor_space_transformer_v1::{
                JayHeadExtCompositorSpaceTransformerV1RequestHandler, SetTransform,
                SupportedTransform,
            },
            jay_head_manager_ext_compositor_space_transformer_v1::JayHeadManagerExtCompositorSpaceTransformerV1RequestHandler,
        },
    },
    jay_config::video::Transform,
    std::rc::Rc,
};

impl_compositor_space_transformer_v1! {
    version = 1,
    after_announce = after_announce,
}

impl HeadName {
    fn after_announce(&self, _shared: &HeadState) {
        self.send_supported_transform(Transform::None);
        self.send_supported_transform(Transform::Rotate90);
        self.send_supported_transform(Transform::Rotate180);
        self.send_supported_transform(Transform::Rotate270);
        self.send_supported_transform(Transform::Flip);
        self.send_supported_transform(Transform::FlipRotate90);
        self.send_supported_transform(Transform::FlipRotate180);
        self.send_supported_transform(Transform::FlipRotate270);
    }

    fn send_supported_transform(&self, transform: Transform) {
        self.client.event(SupportedTransform {
            self_id: self.id,
            transform: transform.to_wl() as _,
        });
    }
}

impl JayHeadManagerExtCompositorSpaceTransformerV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCompositorSpaceTransformerV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        let Some(transform) = Transform::from_wl(req.transform as _) else {
            return Err(ErrorName::UnknownTransform(req.transform));
        };
        self.common.push_op(HeadOp::SetTransform(transform))?;
        Ok(())
    }
}

error! {
    #[error("Unknown transform {0}")]
    UnknownTransform(u32),
}
