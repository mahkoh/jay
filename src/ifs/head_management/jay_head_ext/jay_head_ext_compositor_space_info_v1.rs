use {
    crate::{
        client::ClientError,
        ifs::head_management::HeadCommonError,
        scale::Scale,
        state::ConnectorData,
        tree::OutputNode,
        utils::transform_ext::TransformExt,
        wire::{
            jay_head_ext_compositor_space_info_v1::{
                JayHeadExtCompositorSpaceInfoV1RequestHandler, Outside, Position, Scaling, Size,
            },
            jay_head_manager_ext_compositor_space_info_v1::JayHeadManagerExtCompositorSpaceInfoV1RequestHandler,
        },
    },
    jay_config::video::Transform,
    std::rc::Rc,
};

ext! {
    snake = compositor_space_info_v1,
    camel = CompositorSpaceInfoV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtCompositorSpaceInfoV1 {
    fn after_announce(&self, connector: &ConnectorData) {
        if let Some(output) = self.client.state.outputs.get(&connector.id) {
            if let Some(node) = &output.node {
                self.send_inside(node);
            }
        }
    }

    pub fn send_outside(&self) {
        self.client.event(Outside { self_id: self.id });
    }

    pub fn send_inside(&self, node: &OutputNode) {
        let pos = node.global.pos.get();
        self.send_position(pos.x1(), pos.y1());
        self.send_size(pos.width(), pos.height());
        self.send_transform(node.global.persistent.transform.get());
        self.send_scaling(node.global.persistent.scale.get());
    }

    pub fn send_position(&self, x: i32, y: i32) {
        self.client.event(Position {
            self_id: self.id,
            x,
            y,
        });
    }

    pub fn send_size(&self, width: i32, height: i32) {
        self.client.event(Size {
            self_id: self.id,
            width,
            height,
        });
    }

    pub fn send_transform(&self, transform: Transform) {
        self.client.event(
            crate::wire::jay_head_ext_compositor_space_info_v1::Transform {
                self_id: self.id,
                transform: transform.to_wl() as _,
            },
        );
    }

    pub fn send_scaling(&self, scale: Scale) {
        self.client.event(Scaling {
            self_id: self.id,
            scaling: scale.to_wl(),
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
