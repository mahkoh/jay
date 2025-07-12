use {
    super::HeadMgrCommon,
    crate::{
        backend::transaction::{ConnectorTransaction, PreparedConnectorTransaction},
        client::{Client, ClientError},
        ifs::{
            head_management::{
                Head, HeadCommon, HeadCommonError, HeadMgrState, HeadName, HeadOp,
                HeadTransactionError,
                head_management_macros::{HeadExtension, MgrExts, announce_head, bind_extension},
                jay_head_manager_v1::JayHeadManagerV1,
                jay_head_transaction_result_v1::JayHeadTransactionResultV1,
                jay_head_v1::JayHeadV1,
            },
            wl_output::PersistentOutputState,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::{ConnectorData, State},
        tree::{TearingMode, VrrMode},
        utils::{copyhashmap::CopyHashMap, numcell::NumCell},
        wire::{
            JayHeadManagerSessionV1Id, JayHeadTransactionResultV1Id,
            jay_head_manager_session_v1::{
                ApplyChanges, BeginTransaction, BindExtension, CommitTransaction, Destroy,
                HeadComplete, HeadStart, JayHeadManagerSessionV1RequestHandler,
                RollbackTransaction, Start, Stop, Stopped, TestTransaction, TransactionEnded,
                TransactionStarted,
            },
        },
    },
    linearize::LinearizeExt,
    std::{cell::Cell, mem, ops::Deref, rc::Rc},
    thiserror::Error,
};

pub struct HeadManagerEvent {
    mgr: Rc<JayHeadManagerSessionV1>,
    ty: HeadManagerEventType,
}

pub(super) enum HeadManagerEventType {
    Done,
    TransactionStarted,
    TransactionEnded,
    TransactionResult(Rc<JayHeadTransactionResultV1>),
    Stopped,
}

pub async fn handle_jay_head_manager_done(state: Rc<State>) {
    loop {
        let ev = state.head_managers_async.pop().await;
        let session = ev.mgr;
        if session.mgr.destroyed.get() {
            continue;
        }
        if session.mgr.done_scheduled.take() {
            session.mgr.send_done();
        }
        if session.common.state.get() == HeadMgrState::Stopped {
            continue;
        }
        match ev.ty {
            HeadManagerEventType::Done => {}
            HeadManagerEventType::TransactionStarted => {
                session.send_transaction_started();
            }
            HeadManagerEventType::TransactionEnded => {
                session.send_transaction_ended();
            }
            HeadManagerEventType::Stopped => {
                session.common.state.set(HeadMgrState::Stopped);
                session.send_stopped();
            }
            HeadManagerEventType::TransactionResult(res) => {
                if !res.destroyed.get() {
                    res.send();
                }
            }
        }
    }
}

pub struct JayHeadManagerSessionV1 {
    pub(super) id: JayHeadManagerSessionV1Id,
    pub(super) mgr: Rc<JayHeadManagerV1>,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub(super) common: Rc<HeadMgrCommon>,
    pub(super) serial: NumCell<u64>,
    pub(super) heads: CopyHashMap<HeadName, Rc<Head>>,
    pub(super) ext: MgrExts,
}

impl JayHeadManagerSessionV1 {
    pub fn announce(self: &Rc<Self>, connector: &ConnectorData) {
        if self.common.in_transaction.get() {
            return;
        }
        if let Err(e) = self.try_announce(connector) {
            self.client.error(e);
        }
    }

    fn try_announce(self: &Rc<Self>, connector: &ConnectorData) -> Result<(), ClientError> {
        let shared = connector.head_managers.state.clone();
        let common = Rc::new(HeadCommon {
            mgr: self.common.clone(),
            name: connector.head_managers.name,
            id: connector.id,
            removed: Cell::new(false),
            shared: shared.clone(),
            snapshot_state: shared.deref().clone(),
            transaction_state: shared.deref().clone(),
            pending: Default::default(),
        });
        let obj = Rc::new(JayHeadV1 {
            id: self.client.new_id()?,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            common: common.clone(),
        });
        track!(self.client, obj);
        self.client.add_server_obj(&obj);
        let head = announce_head(self, &obj, connector)?;
        connector
            .head_managers
            .managers
            .set((self.client.id, self.id), head.clone());
        self.heads.set(common.name, head);
        Ok(())
    }

    fn schedule_event(self: &Rc<Self>, ty: HeadManagerEventType) {
        self.client
            .state
            .head_managers_async
            .push(HeadManagerEvent {
                mgr: self.clone(),
                ty,
            });
    }

    pub(super) fn schedule_done(self: &Rc<Self>) {
        if !self.mgr.done_scheduled.replace(true) {
            self.serial.fetch_add(1);
            self.schedule_event(HeadManagerEventType::Done);
        }
    }

    fn schedule_transaction_started(self: &Rc<Self>) {
        self.mgr.done_scheduled.set(true);
        self.schedule_event(HeadManagerEventType::TransactionStarted);
    }

    fn schedule_transaction_ended(self: &Rc<Self>) {
        self.mgr.done_scheduled.set(true);
        self.schedule_event(HeadManagerEventType::TransactionEnded);
    }

    fn schedule_stopped(self: &Rc<Self>) {
        self.mgr.done_scheduled.set(true);
        self.schedule_event(HeadManagerEventType::Stopped);
    }

    fn send_transaction_started(&self) {
        self.client.event(TransactionStarted { self_id: self.id });
    }

    fn send_transaction_ended(&self) {
        self.client.event(TransactionEnded { self_id: self.id });
    }

    fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }

    pub(super) fn send_head_start(&self, head: &JayHeadV1, name: HeadName) {
        self.client.event(HeadStart {
            self_id: self.id,
            head: head.id,
            name: name.0,
        });
    }

    pub(super) fn send_head_complete(&self) {
        self.client.event(HeadComplete { self_id: self.id });
    }

    fn detach(&self, send_removed: bool) {
        let id = (self.client.id, self.id);
        for data in self.client.state.connectors.lock().values() {
            if let Some(head) = data.head_managers.managers.remove(&id) {
                if send_removed {
                    head.head.send_removed();
                }
            }
        }
        self.client.state.head_managers.remove(&id);
        self.heads.clear();
    }

    fn schedule_transaction_result(
        self: &Rc<Self>,
        id: JayHeadTransactionResultV1Id,
        error: Option<HeadTransactionError>,
    ) -> Result<(), JayHeadManagerSessionV1Error> {
        if error.is_some() {
            self.common.transaction_failed.set(true);
        }
        self.mgr.done_scheduled.set(true);
        let res = Rc::new(JayHeadTransactionResultV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            error: error.map(Rc::new),
            destroyed: Cell::new(false),
        });
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        self.schedule_event(HeadManagerEventType::TransactionResult(res));
        Ok(())
    }

    fn after_transaction_ended(self: &Rc<Self>) {
        let mut to_remove = vec![];
        for head in self.heads.lock().values() {
            if self.client.state.connectors.not_contains(&head.common.id) {
                head.head.send_removed();
                to_remove.push(head.common.name);
                continue;
            }
            let _ = head.common.pending.take();
            let tran = &*head.common.transaction_state.borrow();
            let shared = &*head.common.shared.borrow();
            head.ext.after_transaction(shared, tran);
        }
        for name in to_remove {
            self.heads.remove(&name);
        }
        for connector in self.client.state.connectors.lock().values() {
            if !self.heads.contains(&connector.head_managers.name) {
                self.announce(connector);
            }
        }
        self.schedule_transaction_ended();
    }

    fn prepare_transaction(&self) -> Result<PreparedConnectorTransaction, HeadTransactionError> {
        let mut tran = ConnectorTransaction::new(&self.client.state);
        for head in self.heads.lock().values() {
            let current = &*head.common.shared.borrow();
            let snapshot = &*head.common.snapshot_state.borrow();
            let desired = &*head.common.transaction_state.borrow();
            if desired == current || desired == snapshot {
                continue;
            }
            let Some(connector) = self.client.state.connectors.get(&head.common.id) else {
                return Err(HeadTransactionError::HeadRemoved(head.common.id));
            };
            let old = connector.state.get();
            #[expect(unused_mut)]
            let mut new = old;
            if old == new {
                continue;
            }
            if current.monitor_info != desired.monitor_info {
                return Err(HeadTransactionError::MonitorChanged(head.common.id));
            }
            tran.add(&connector.connector, new)?;
        }
        Ok(tran.prepare()?)
    }

    fn commit_transaction(&self) -> Result<(), HeadTransactionError> {
        self.prepare_transaction()?.apply()?.commit();
        Ok(())
    }
}

impl JayHeadManagerSessionV1RequestHandler for JayHeadManagerSessionV1 {
    type Error = JayHeadManagerSessionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_stopped()?;
        self.mgr.sessions.fetch_sub(1);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn bind_extension(&self, req: BindExtension<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.get() != HeadMgrState::Init {
            return Err(JayHeadManagerSessionV1Error::AlreadyStarted);
        }
        let Some(ext) = HeadExtension::from_linear(req.name as usize) else {
            return Err(JayHeadManagerSessionV1Error::UnknownExtension(req.name));
        };
        bind_extension(self, ext, req.name, req.version, req.id)?;
        Ok(())
    }

    fn stop(&self, _req: Stop, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.in_transaction.get() {
            return Err(JayHeadManagerSessionV1Error::InTransaction);
        }
        match self.common.state.get() {
            HeadMgrState::Init | HeadMgrState::Started => {}
            HeadMgrState::StopScheduled | HeadMgrState::Stopped => return Ok(()),
        }
        self.common.state.set(HeadMgrState::StopScheduled);
        self.detach(true);
        self.serial.fetch_add(1);
        slf.schedule_stopped();
        Ok(())
    }

    fn start(&self, _req: Start, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.get() != HeadMgrState::Init {
            return Err(JayHeadManagerSessionV1Error::AlreadyStarted);
        }
        self.common.state.set(HeadMgrState::Started);
        self.client
            .state
            .head_managers
            .set((self.client.id, self.id), slf.clone());
        for connector in self.client.state.connectors.lock().values() {
            slf.announce(connector);
        }
        slf.schedule_done();
        Ok(())
    }

    fn begin_transaction(&self, _req: BeginTransaction, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.in_transaction.replace(true) {
            return Err(JayHeadManagerSessionV1Error::AlreadyInTransaction);
        }
        for head in self.heads.lock().values() {
            let snapshot = head.common.shared.borrow().clone();
            *head.common.transaction_state.borrow_mut() = snapshot.clone();
            *head.common.snapshot_state.borrow_mut() = snapshot;
            head.common.pending.borrow_mut().clear();
        }
        self.common.transaction_failed.set(false);
        slf.schedule_transaction_started();
        Ok(())
    }

    fn rollback_transaction(
        &self,
        _req: RollbackTransaction,
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if !self.common.in_transaction.replace(false) {
            return Err(JayHeadManagerSessionV1Error::NotInTransaction);
        }
        slf.after_transaction_ended();
        Ok(())
    }

    fn apply_changes(&self, req: ApplyChanges, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.common.in_transaction.get() {
            return Err(JayHeadManagerSessionV1Error::NotInTransaction);
        }
        macro_rules! schedule_result {
            ($res:expr) => {
                slf.schedule_transaction_result(req.result, Some($res))?;
                return Ok(());
            };
        }
        if self.common.transaction_failed.get() {
            schedule_result!(HeadTransactionError::AlreadyFailed);
        }
        bitflags! {
            ToSend: u32;
            CORE_INFO                       = 1 << 0,
            COMPOSITOR_SPACE_INFO_FULL      = 1 << 1,
            COMPOSITOR_SPACE_INFO_POS       = 1 << 2,
            COMPOSITOR_SPACE_INFO_SIZE      = 1 << 3,
            COMPOSITOR_SPACE_INFO_TRANSFORM = 1 << 4,
            COMPOSITOR_SPACE_INFO_SCALE     = 1 << 5,
            COMPOSITOR_SPACE_INFO_ENABLED   = 1 << 13,
        }
        for head in self.heads.lock().values() {
            let pending = mem::take(&mut *head.common.pending.borrow_mut());
            #[expect(unused_variables)]
            let snapshot = &*head.common.snapshot_state.borrow();
            let state = &mut *head.common.transaction_state.borrow_mut();
            let mut to_send = ToSend::default();
            for op in pending {
                match op {
                    HeadOp::SetPosition(x, y) => {
                        state.position = (x, y);
                        to_send |= COMPOSITOR_SPACE_INFO_POS;
                    }
                    HeadOp::SetScale(s) => {
                        state.scale = s;
                        state.update_size();
                        to_send |= COMPOSITOR_SPACE_INFO_SCALE;
                        to_send |= COMPOSITOR_SPACE_INFO_SIZE;
                    }
                }
            }
            if to_send.contains(CORE_INFO)
                && let Some(i) = &head.ext.core_info_v1
            {
                i.send_wl_output(state);
            }
            if let Some(i) = &head.ext.compositor_space_info_v1 {
                if to_send.contains(COMPOSITOR_SPACE_INFO_ENABLED) {
                    i.send_enabled(state);
                }
                if to_send.contains(COMPOSITOR_SPACE_INFO_FULL) {
                    i.send_inside_outside(state);
                } else {
                    if to_send.contains(COMPOSITOR_SPACE_INFO_POS) {
                        i.send_position(state);
                    }
                    if to_send.contains(COMPOSITOR_SPACE_INFO_SIZE) {
                        i.send_size(state);
                    }
                    if to_send.contains(COMPOSITOR_SPACE_INFO_TRANSFORM) {
                        i.send_transform(state);
                    }
                    if to_send.contains(COMPOSITOR_SPACE_INFO_SCALE) {
                        i.send_scale(state);
                    }
                }
            }
        }
        slf.schedule_transaction_result(req.result, None)?;
        Ok(())
    }

    fn test_transaction(&self, req: TestTransaction, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.common.in_transaction.get() {
            return Err(JayHeadManagerSessionV1Error::NotInTransaction);
        }
        let res = self.prepare_transaction().err();
        slf.schedule_transaction_result(req.result, res)?;
        Ok(())
    }

    fn commit_transaction(
        &self,
        req: CommitTransaction,
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if !self.common.in_transaction.replace(false) {
            return Err(JayHeadManagerSessionV1Error::NotInTransaction);
        }
        slf.after_transaction_ended();
        if let Err(e) = self.commit_transaction() {
            slf.schedule_transaction_result(req.result, Some(e))?;
            return Ok(());
        }
        for head in self.heads.lock().values() {
            let desired = &*head.common.transaction_state.borrow();
            if let Some(output) = self.client.state.outputs.get(&head.common.id)
                && let Some(node) = &output.node
            {
                node.set_position(desired.position.0, desired.position.1);
                node.set_preferred_scale(desired.scale);
            } else if let Some(mi) = &desired.monitor_info {
                let pos = &self.client.state.persistent_output_states;
                let pos = match pos.get(&mi.output_id) {
                    Some(ps) => ps,
                    _ => {
                        let ps = Rc::new(PersistentOutputState {
                            transform: Default::default(),
                            scale: Default::default(),
                            pos: Default::default(),
                            vrr_mode: Cell::new(&VrrMode::Never),
                            vrr_cursor_hz: Default::default(),
                            tearing_mode: Cell::new(&TearingMode::Never),
                            brightness: Default::default(),
                        });
                        pos.set(mi.output_id.clone(), ps.clone());
                        ps
                    }
                };
                pos.pos.set(desired.position);
                pos.scale.set(desired.scale);
            }
        }
        slf.schedule_transaction_result(req.result, None)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadManagerSessionV1;
    version = self.version;
}

impl Object for JayHeadManagerSessionV1 {
    fn break_loops(&self) {
        self.detach(false);
    }
}

simple_add_obj!(JayHeadManagerSessionV1);

#[derive(Debug, Error)]
pub enum JayHeadManagerSessionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Common(#[from] HeadCommonError),
    #[error("Manager was already started")]
    AlreadyStarted,
    #[error("There is no extension with name {}", .0)]
    UnknownExtension(u32),
    #[error("The extension with name {} is already bound", .0)]
    AlreadyBound(u32),
    #[error("The extension with name {} does not support version {}", .0, .1 .0)]
    UnsupportedVersion(u32, Version),
    #[error("There already is an active transaction")]
    AlreadyInTransaction,
    #[error("There is no active transaction")]
    NotInTransaction,
    #[error("There is an active transaction")]
    InTransaction,
}
efrom!(JayHeadManagerSessionV1Error, ClientError);
