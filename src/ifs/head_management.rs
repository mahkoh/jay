use {
    crate::{
        backend::{Mode, MonitorInfo},
        client::ClientId,
        globals::GlobalName,
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
            jay_head_manager_session_v1::JayHeadManagerSessionV1,
            jay_head_v1::JayHeadV1,
        },
        scale::Scale,
        state::OutputData,
        tree::OutputNode,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt},
        wire::JayHeadManagerSessionV1Id,
    },
    jay_config::video::Transform,
    linearize::Linearize,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

pub mod jay_head_error_v1;
mod jay_head_ext;
pub mod jay_head_manager_session_v1;
pub mod jay_head_manager_v1;
mod jay_head_transaction_result_v1;
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
    pub mgr: Rc<JayHeadManagerSessionV1>,
    pub common: Rc<HeadCommon>,
    pub head: Rc<JayHeadV1>,
    pub core_info_v1: Option<Rc<JayHeadExtCoreInfoV1>>,
    pub compositor_space_info_v1: Option<Rc<JayHeadExtCompositorSpaceInfoV1>>,
    pub compositor_space_positioner_v1: Option<Rc<JayHeadExtCompositorSpacePositionerV1>>,
    pub compositor_space_transformer_v1: Option<Rc<JayHeadExtCompositorSpaceTransformerV1>>,
    pub physical_display_info_v1: Option<Rc<JayHeadExtPhysicalDisplayInfoV1>>,
    pub connector_info_v1: Option<Rc<JayHeadExtConnectorInfoV1>>,
    pub connector_settings_v1: Option<Rc<JayHeadExtConnectorSettingsV1>>,
}

pub enum HeadTransactionResult {
    Success,
    Failed,
    PositionOutOfBounds,
}

pub struct HeadCommon {
    pub mgr: Rc<HeadMgrCommon>,
    pub name: HeadName,
    pub removed: Cell<bool>,

    pub shared: Rc<RefCell<HeadState>>,
    pub snapshot_state: RefCell<HeadState>,
    pub transaction_state: RefCell<HeadState>,
    pub pending: RefCell<Vec<HeadOp>>,
}

#[derive(Clone, Default)]
pub struct HeadState {
    pub name: Rc<String>,
    pub wl_output: Option<GlobalName>,
    pub connector_enabled: bool,
    pub connected: bool,
    pub in_compositor_space: bool,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub mode: Mode,
    pub transform: Transform,
    pub scale: Scale,
    pub monitor_info: Option<Rc<MonitorInfo>>,
}

pub enum HeadOp {
    SetPosition(i32, i32),
    SetConnectorEnabled(bool),
    SetTransform(Transform),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum HeadMgrState {
    #[default]
    Init,
    Started,
    StopScheduled,
    Stopped,
}

pub struct HeadMgrCommon {
    pub state: Cell<HeadMgrState>,
    pub in_transaction: Cell<bool>,
    pub transaction_failed: Cell<bool>,
}

impl HeadCommon {
    pub fn assert_removed(&self) -> Result<(), HeadCommonError> {
        if self.removed.get() {
            Ok(())
        } else {
            Err(HeadCommonError::NotYetRemoved)
        }
    }

    pub fn assert_in_transaction(&self) -> Result<(), HeadCommonError> {
        if self.mgr.in_transaction.get() {
            Ok(())
        } else {
            Err(HeadCommonError::NotInTransaction)
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
    #[error("Manager is not inside a transaction")]
    NotInTransaction,
    #[error("Manager has not yet been stopped")]
    NotYetStopped,
}

pub struct HeadManagers {
    name: HeadName,
    state: Rc<RefCell<HeadState>>,
    managers: CopyHashMap<(ClientId, JayHeadManagerSessionV1Id), Rc<Head>>,
}

macro_rules! skip_in_transaction {
    ($mgr:expr) => {
        if $mgr.common.mgr.in_transaction.get() {
            continue;
        }
    };
}

impl HeadManagers {
    pub fn new(name: HeadName, state: HeadState) -> Self {
        Self {
            name,
            state: Rc::new(RefCell::new(state)),
            managers: Default::default(),
        }
    }

    pub fn handle_removed(&self) {
        for mgr in self.managers.lock().drain_values() {
            mgr.mgr.heads.remove(&self.name);
            mgr.head.send_removed();
            mgr.mgr.schedule_done();
        }
    }

    pub fn handle_output_connected(&self, output: &OutputData) {
        let state = &mut *self.state.borrow_mut();
        state.connected = true;
        state.monitor_info = Some(output.monitor_info.clone());
        if let Some(n) = &output.node {
            state.wl_output = Some(n.global.name);
            state.in_compositor_space = true;
            state.position = n.global.pos.get().position();
            state.size = n.global.pos.get().size();
            state.mode = n.global.mode.get();
            state.transform = n.global.persistent.transform.get();
        }
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.connector_info_v1 {
                ext.send_connected(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.physical_display_info_v1 {
                ext.send_info(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_inside_outside(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.core_info_v1 {
                ext.send_wl_output(state);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_output_disconnected(&self) {
        let state = &mut *self.state.borrow_mut();
        state.connected = false;
        state.in_compositor_space = false;
        state.wl_output = None;
        state.monitor_info = None;
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_inside_outside(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.connector_info_v1 {
                ext.send_connected(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.core_info_v1 {
                ext.send_wl_output(state);
                mgr.mgr.schedule_done();
            }
            if let Some(ext) = &mgr.physical_display_info_v1 {
                ext.send_info(state);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_position_size_change(&self, node: &OutputNode) {
        let state = &mut *self.state.borrow_mut();
        let pos = node.global.pos.get();
        state.position = pos.position();
        state.size = pos.size();
        state.mode = node.global.mode.get();
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_position(state);
                ext.send_size(state);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_transform_change(&self, transform: Transform) {
        let state = &mut *self.state.borrow_mut();
        state.transform = transform;
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_transform(state);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_enabled_change(&self, enabled: bool) {
        let state = &mut *self.state.borrow_mut();
        state.connector_enabled = enabled;
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.connector_info_v1 {
                ext.send_enabled(state);
                mgr.mgr.schedule_done();
            }
        }
    }

    pub fn handle_scale_change(&self, scale: Scale) {
        let state = &mut *self.state.borrow_mut();
        state.scale = scale;
        for mgr in self.managers.lock().values() {
            skip_in_transaction!(mgr);
            if let Some(ext) = &mgr.compositor_space_info_v1 {
                ext.send_scale(state);
                mgr.mgr.schedule_done();
            }
        }
    }
}
