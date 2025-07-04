use {
    crate::{
        client::ClientId,
        ifs::head_management::{
            jay_head_ext::{
                jay_head_ext_compositor_space_info_v1::JayHeadExtCompositorSpaceInfoV1,
                jay_head_ext_compositor_space_positioner_v1::JayHeadExtCompositorSpacePositionerV1,
                jay_head_ext_compositor_space_transformer_v1::JayHeadExtCompositorSpaceTransformerV1,
                jay_head_ext_connector_info_v1::JayHeadExtConnectorInfoV1,
                jay_head_ext_connector_settings_v1::JayHeadExtConnectorSettingsV1,
                jay_head_ext_core_info_v1::JayHeadExtCoreInfoV1,
                jay_head_ext_physical_display_info_v1::JayHeadExtPhysicalDisplayInfoV1,
            },
            jay_head_manager_v1::JayHeadManagerV1,
            jay_head_v1::JayHeadV1,
        },
        rect::Rect,
        scale::Scale,
        state::OutputData,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt},
        wire::JayHeadManagerV1Id,
    },
    ahash::AHashMap,
    jay_config::video::Transform,
    linearize::Linearize,
    std::{
        cell::{Cell, RefCell, RefMut},
        rc::Rc,
    },
    thiserror::Error,
};

pub mod jay_head_error_v1;
mod jay_head_ext;
pub mod jay_head_manager_v1;
mod jay_head_transaction_result_v1;
pub mod jay_head_transaction_v1;
mod jay_head_v1;

linear_ids!(HeadNames, HeadName, u64);

#[derive(Linearize)]
enum HeadExtension {
    CoreInfoV1,
    CompositorSpaceInfoV1,
    CompositorSpacePositionerV1,
    CompositorSpaceTransformerV1,
    ConnectorInfoV1,
    ConnectorSettingsV1,
    PhysicalDisplayInfoV1,
}

pub struct Head {
    pub mgr: Rc<JayHeadManagerV1>,
    pub head: Rc<JayHeadV1>,
    pub core_info_v1: Option<Rc<JayHeadExtCoreInfoV1>>,
    pub compositor_space_info_v1: Option<Rc<JayHeadExtCompositorSpaceInfoV1>>,
    pub compositor_space_positioner_v1: Option<Rc<JayHeadExtCompositorSpacePositionerV1>>,
    pub compositor_space_transformer_v1: Option<Rc<JayHeadExtCompositorSpaceTransformerV1>>,
    pub physical_display_info_v1: Option<Rc<JayHeadExtPhysicalDisplayInfoV1>>,
    pub connector_info_v1: Option<Rc<JayHeadExtConnectorInfoV1>>,
    pub connector_settings_v1: Option<Rc<JayHeadExtConnectorSettingsV1>>,
}

pub struct HeadCommon {
    pub name: HeadName,
    pub removed: Cell<bool>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum HeadMgrState {
    #[default]
    Init,
    Started,
    Stopped,
}

pub struct HeadMgrCommon {
    pub state: Cell<HeadMgrState>,
}

#[derive(Default)]
pub struct HeadTransaction {
    pub outputs: RefCell<AHashMap<HeadName, HeadTransactionHead>>,
}

#[derive(Default)]
pub struct HeadTransactionHead {
    pub position: Option<(i32, i32)>,
    pub connector_enabled: Option<bool>,
    pub transform: Option<Transform>,
}

impl HeadTransaction {
    pub fn get_or_create(&self, head: HeadName) -> RefMut<'_, HeadTransactionHead> {
        RefMut::map(self.outputs.borrow_mut(), |map| {
            map.entry(head).or_default()
        })
    }
}

impl HeadCommon {
    pub fn assert_removed(&self) -> Result<(), HeadCommonError> {
        if self.removed.get() {
            Ok(())
        } else {
            Err(HeadCommonError::NotYetRemoved)
        }
    }
}

impl HeadMgrCommon {
    pub fn assert_stopped(&self) -> Result<(), HeadCommonError> {
        if self.state.get() == HeadMgrState::Stopped {
            Ok(())
        } else {
            Err(HeadCommonError::NotYetStopped)
        }
    }
}

#[derive(Debug, Error)]
pub enum HeadCommonError {
    #[error("Head has not yet been removed")]
    NotYetRemoved,
    #[error("Manager has not yet been stopped")]
    NotYetStopped,
}

#[derive(Default)]
pub struct HeadManagers {
    managers: CopyHashMap<(ClientId, JayHeadManagerV1Id), Rc<Head>>,
}

impl HeadManagers {
    pub fn handle_removed(&self) {
        for mgr in self.managers.lock().drain_values() {
            mgr.head.send_removed();
            mgr.mgr.schedule_done();
        }
    }

    pub fn handle_output_connected(&self, output: &OutputData) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.connector_info_v1 {
                ext.send_connected();
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.physical_display_info_v1 {
                ext.connected(&output.monitor_info);
                mgr.mgr.schedule_done();
            }
            if let Some(node) = &output.node {
                if let Some(ext) = &mgr.compositor_space_info_v1 {
                    ext.send_inside(node);
                    mgr.mgr.schedule_done();
                }
                if let Some(ext) = &mgr.core_info_v1 {
                    ext.send_wl_output(node.global.name);
                    mgr.mgr.schedule_done();
                }
            }
        }
    }

    pub fn handle_output_disconnected(&self) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_outside();
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.connector_info_v1 {
                ext.send_disconnected();
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.core_info_v1 {
                ext.send_no_wl_output();
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.physical_display_info_v1 {
                ext.send_reset();
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_position_size_change(&self, rect: &Rect) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_position(rect.x1(), rect.y1());
                ext.send_size(rect.width(), rect.height());
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_transform_change(&self, transform: Transform) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_transform(transform);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_enabled_change(&self, enabled: bool) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.connector_info_v1 {
                if enabled {
                    ext.send_enabled();
                } else {
                    ext.send_disabled();
                }
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_scale_change(&self, scale: Scale) {
        for mgr in self.managers.lock().values() {
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_scaling(scale);
                mgr.mgr.schedule_done();
            }
        }
    }
}
