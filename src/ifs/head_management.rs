use {
    crate::{
        backend::{ConnectorId, Mode, MonitorInfo, transaction::BackendConnectorTransactionError},
        client::ClientId,
        globals::GlobalName,
        ifs::head_management::{
            head_management_macros::HeadExts, jay_head_manager_session_v1::JayHeadManagerSessionV1,
            jay_head_v1::JayHeadV1,
        },
        scale::Scale,
        state::OutputData,
        tree::OutputNode,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt, rc_eq::RcEq},
        wire::JayHeadManagerSessionV1Id,
    },
    jay_config::video::Transform,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

#[macro_use]
mod head_management_macros;
pub mod jay_head_error_v1;
mod jay_head_ext;
pub mod jay_head_manager_session_v1;
pub mod jay_head_manager_v1;
mod jay_head_transaction_result_v1;
mod jay_head_v1;

linear_ids!(HeadNames, HeadName, u64);

struct Head {
    session: Rc<JayHeadManagerSessionV1>,
    common: Rc<HeadCommon>,
    head: Rc<JayHeadV1>,
    ext: HeadExts,
}

#[derive(Error, Debug)]
enum HeadTransactionError {
    #[error("The connector {} has been removed", .0)]
    HeadRemoved(ConnectorId),
    #[error("The display connected to connector {} has changed", .0)]
    MonitorChanged(ConnectorId),
    #[error("The transaction has already failed")]
    AlreadyFailed,
    #[error(transparent)]
    Backend(#[from] BackendConnectorTransactionError),
}

struct HeadCommon {
    mgr: Rc<HeadMgrCommon>,
    name: HeadName,
    id: ConnectorId,
    removed: Cell<bool>,

    shared: Rc<RefCell<HeadState>>,
    snapshot_state: RefCell<HeadState>,
    transaction_state: RefCell<HeadState>,
    pending: RefCell<Vec<HeadOp>>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct HeadState {
    pub name: RcEq<String>,
    pub wl_output: Option<GlobalName>,
    pub connector_enabled: bool,
    pub in_compositor_space: bool,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub mode: Mode,
    pub transform: Transform,
    pub scale: Scale,
    pub monitor_info: Option<RcEq<MonitorInfo>>,
}

impl HeadState {
    fn update_in_compositor_space(&mut self, wl_output: Option<GlobalName>) {
        self.in_compositor_space = false;
        self.wl_output = None;
        if !self.connector_enabled {
            return;
        }
        let Some(mi) = &self.monitor_info else {
            return;
        };
        if mi.non_desktop {
            return;
        }
        self.in_compositor_space = true;
        self.wl_output = wl_output;
    }

    fn update_size(&mut self) {
        self.size =
            OutputNode::calculate_extents_(self.mode, self.transform, self.scale, self.position)
                .size();
    }
}

enum HeadOp {
    SetPosition(i32, i32),
    SetScale(Scale),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum HeadMgrState {
    #[default]
    Init,
    Started,
    StopScheduled,
    Stopped,
}

struct HeadMgrCommon {
    state: Cell<HeadMgrState>,
    in_transaction: Cell<bool>,
    transaction_failed: Cell<bool>,
}

impl HeadCommon {
    fn assert_removed(&self) -> Result<(), HeadCommonError> {
        if self.removed.get() {
            Ok(())
        } else {
            Err(HeadCommonError::NotYetRemoved)
        }
    }

    fn assert_in_transaction(&self) -> Result<(), HeadCommonError> {
        if self.mgr.in_transaction.get() {
            Ok(())
        } else {
            Err(HeadCommonError::NotInTransaction)
        }
    }

    fn push_op(&self, op: HeadOp) -> Result<(), HeadCommonError> {
        self.assert_in_transaction()?;
        self.pending.borrow_mut().push(op);
        Ok(())
    }
}

impl HeadMgrCommon {
    fn assert_stopped(&self) -> Result<(), HeadCommonError> {
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
        for head in self.managers.lock().drain_values() {
            skip_in_transaction!(head);
            head.session.heads.remove(&self.name);
            head.head.send_removed();
            head.session.schedule_done();
        }
    }

    pub fn handle_output_connected(&self, output: &OutputData) {
        let state = &mut *self.state.borrow_mut();
        state.monitor_info = Some(RcEq(output.monitor_info.clone()));
        state.update_in_compositor_space(output.node.as_ref().map(|n| n.global.name));
        if let Some(n) = &output.node {
            state.position = n.global.pos.get().position();
            state.size = n.global.pos.get().size();
            state.mode = n.global.mode.get();
            state.transform = n.global.persistent.transform.get();
        }
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_inside_outside(state);
                head.session.schedule_done();
            }
            if let Some(ext) = &head.ext.core_info_v1 {
                ext.send_wl_output(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_output_disconnected(&self) {
        let state = &mut *self.state.borrow_mut();
        state.monitor_info = None;
        state.update_in_compositor_space(None);
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_inside_outside(state);
                head.session.schedule_done();
            }
            if let Some(ext) = &head.ext.core_info_v1 {
                ext.send_wl_output(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_position_size_change(&self, node: &OutputNode) {
        let state = &mut *self.state.borrow_mut();
        let pos = node.global.pos.get();
        state.position = pos.position();
        state.size = pos.size();
        state.mode = node.global.mode.get();
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_position(state);
                ext.send_size(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_transform_change(&self, transform: Transform) {
        let state = &mut *self.state.borrow_mut();
        state.transform = transform;
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_transform(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_scale_change(&self, scale: Scale) {
        let state = &mut *self.state.borrow_mut();
        state.scale = scale;
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_scale(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_enabled_change(&self, enabled: bool) {
        let state = &mut *self.state.borrow_mut();
        state.connector_enabled = enabled;
        state.update_in_compositor_space(state.wl_output);
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.compositor_space_info_v1 {
                ext.send_enabled(state);
                ext.send_inside_outside(state);
                head.session.schedule_done();
            }
        }
    }
}
