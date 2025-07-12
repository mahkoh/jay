use {
    crate::{
        backend::{ConnectorId, MonitorInfo, transaction::BackendConnectorTransactionError},
        client::ClientId,
        globals::GlobalName,
        ifs::head_management::{
            head_management_macros::HeadExts, jay_head_manager_session_v1::JayHeadManagerSessionV1,
            jay_head_v1::JayHeadV1,
        },
        state::OutputData,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt, rc_eq::RcEq},
        wire::JayHeadManagerSessionV1Id,
    },
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
    pub monitor_info: Option<RcEq<MonitorInfo>>,
}

enum HeadOp {}

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

    #[expect(dead_code)]
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
        if let Some(n) = &output.node {
            state.wl_output = Some(n.global.name);
        }
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.core_info_v1 {
                ext.send_wl_output(state);
                head.session.schedule_done();
            }
        }
    }

    pub fn handle_output_disconnected(&self) {
        let state = &mut *self.state.borrow_mut();
        state.wl_output = None;
        state.monitor_info = None;
        for head in self.managers.lock().values() {
            skip_in_transaction!(head);
            if let Some(ext) = &head.ext.core_info_v1 {
                ext.send_wl_output(state);
                head.session.schedule_done();
            }
        }
    }
}
