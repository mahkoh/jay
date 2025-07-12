use {
    crate::{
        ifs::head_management::HeadState,
        utils::transform_ext::TransformExt,
        wire::{
            jay_head_ext_compositor_space_info_v1::{
                Disabled, Enabled, Inside, JayHeadExtCompositorSpaceInfoV1RequestHandler, Outside,
                Position, Scaling, Size, Transform,
            },
            jay_head_manager_ext_compositor_space_info_v1::JayHeadManagerExtCompositorSpaceInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_compositor_space_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_enabled(shared);
        self.send_inside_outside(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.connector_enabled != tran.connector_enabled {
            self.send_enabled(shared);
        }
        if shared.in_compositor_space != tran.in_compositor_space {
            self.send_inside_outside(shared);
        } else if shared.in_compositor_space {
            if shared.position != tran.position {
                self.send_position(shared);
            }
            if shared.size != tran.size {
                self.send_size(shared);
            }
            if shared.transform != tran.transform {
                self.send_transform(shared);
            }
            if shared.scale != tran.scale {
                self.send_scale(shared);
            }
        }
    }

    pub(in super::super) fn send_inside_outside(&self, state: &HeadState) {
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

    pub(in super::super) fn send_enabled(&self, state: &HeadState) {
        if state.connector_enabled {
            self.client.event(Enabled { self_id: self.id });
        } else {
            self.client.event(Disabled { self_id: self.id });
        }
    }

    pub(in super::super) fn send_position(&self, state: &HeadState) {
        self.client.event(Position {
            self_id: self.id,
            x: state.position.0,
            y: state.position.1,
        });
    }

    pub(in super::super) fn send_size(&self, state: &HeadState) {
        self.client.event(Size {
            self_id: self.id,
            width: state.size.0,
            height: state.size.1,
        });
    }

    pub(in super::super) fn send_transform(&self, state: &HeadState) {
        self.client.event(Transform {
            self_id: self.id,
            transform: state.transform.to_wl() as _,
        });
    }

    pub(in super::super) fn send_scale(&self, state: &HeadState) {
        self.client.event(Scaling {
            self_id: self.id,
            scaling: state.scale.to_wl(),
        });
    }
}

impl JayHeadManagerExtCompositorSpaceInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtCompositorSpaceInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
