use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadOp, HeadState},
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

ext! {
    snake = compositor_space_transformer_v1,
    camel = CompositorSpaceTransformerV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtCompositorSpaceTransformerV1 {
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

impl JayHeadManagerExtCompositorSpaceTransformerV1RequestHandler
    for JayHeadManagerExtCompositorSpaceTransformerV1
{
    type Error = JayHeadExtCompositorSpaceTransformerV1Error;

    ext_common_req!(compositor_space_transformer_v1);
}

impl JayHeadExtCompositorSpaceTransformerV1RequestHandler
    for JayHeadExtCompositorSpaceTransformerV1
{
    type Error = JayHeadExtCompositorSpaceTransformerV1Error;

    head_common_req!(compositor_space_transformer_v1);

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        let Some(transform) = Transform::from_wl(req.transform as _) else {
            return Err(
                JayHeadExtCompositorSpaceTransformerV1Error::UnknownTransform(req.transform),
            );
        };
        self.common
            .pending
            .borrow_mut()
            .push(HeadOp::SetTransform(transform));
        Ok(())
    }
}

error! {
    CompositorSpaceTransformerV1,
    #[error("Unknown transform {0}")]
    UnknownTransform(u32),
}
