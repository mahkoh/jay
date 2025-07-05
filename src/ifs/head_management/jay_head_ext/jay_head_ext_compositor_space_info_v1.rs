use {
    crate::{
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadState},
        utils::transform_ext::TransformExt,
        wire::{
            jay_head_ext_compositor_space_info_v1::{
                Inside, JayHeadExtCompositorSpaceInfoV1RequestHandler, Outside, Position, Scaling,
                Size,
            },
            jay_head_manager_ext_compositor_space_info_v1::JayHeadManagerExtCompositorSpaceInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

ext! {
    snake = compositor_space_info_v1,
    camel = CompositorSpaceInfoV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtCompositorSpaceInfoV1 {
    fn after_announce(&self, shared: &HeadState) {
        self.send_inside_outside(shared);
    }

    pub fn send_inside_outside(&self, state: &HeadState) {
        if state.in_compositor_space {
            self.client.event(Inside { self_id: self.id });
            self.send_position(state);
            self.send_size(state);
            self.send_transform(state);
            self.send_scale(state);
        } else {
            self.client.event(Outside { self_id: self.id });
        }
    }

    pub fn send_position(&self, state: &HeadState) {
        self.client.event(Position {
            self_id: self.id,
            x: state.position.0,
            y: state.position.1,
        });
    }

    pub fn send_size(&self, state: &HeadState) {
        self.client.event(Size {
            self_id: self.id,
            width: state.size.0,
            height: state.size.1,
        });
    }

    pub fn send_transform(&self, state: &HeadState) {
        self.client.event(
            crate::wire::jay_head_ext_compositor_space_info_v1::Transform {
                self_id: self.id,
                transform: state.transform.to_wl() as _,
            },
        );
    }

    pub fn send_scale(&self, state: &HeadState) {
        self.client.event(Scaling {
            self_id: self.id,
            scaling: state.scale.to_wl(),
        });
    }
}

impl JayHeadManagerExtCompositorSpaceInfoV1RequestHandler
    for JayHeadManagerExtCompositorSpaceInfoV1
{
    type Error = JayHeadExtCompositorSpaceInfoV1Error;

    ext_common_req!(compositor_space_info_v1);
}

impl JayHeadExtCompositorSpaceInfoV1RequestHandler for JayHeadExtCompositorSpaceInfoV1 {
    type Error = JayHeadExtCompositorSpaceInfoV1Error;

    head_common_req!(compositor_space_info_v1);
}

error! {
    CompositorSpaceInfoV1
}
