use {
    super::HeadMgrCommon,
    crate::{
        client::{Client, ClientError},
        compositor::MAX_EXTENTS,
        ifs::head_management::{
            Head, HeadCommon, HeadCommonError, HeadExtension, HeadMgrState, HeadName, HeadOp,
            HeadTransactionResult,
            jay_head_ext::{
                jay_head_ext_compositor_space_info_v1::JayHeadManagerExtCompositorSpaceInfoV1,
                jay_head_ext_compositor_space_positioner_v1::JayHeadManagerExtCompositorSpacePositionerV1,
                jay_head_ext_compositor_space_scaler_v1::{MAX_SCALE, MIN_SCALE},
                jay_head_ext_compositor_space_transformer_v1::JayHeadManagerExtCompositorSpaceTransformerV1,
                jay_head_ext_connector_info_v1::JayHeadManagerExtConnectorInfoV1,
                jay_head_ext_connector_settings_v1::JayHeadManagerExtConnectorSettingsV1,
                jay_head_ext_core_info_v1::JayHeadManagerExtCoreInfoV1,
                jay_head_ext_physical_display_info_v1::JayHeadManagerExtPhysicalDisplayInfoV1,
            },
            jay_head_manager_v1::JayHeadManagerV1,
            jay_head_transaction_result_v1::JayHeadTransactionResultV1,
            jay_head_v1::JayHeadV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::{ConnectorData, State},
        tree::OutputNode,
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap, numcell::NumCell, rc_eq::rc_eq},
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
    std::{cell::Cell, mem, rc::Rc},
    thiserror::Error,
};
use crate::ifs::head_management::jay_head_ext::jay_head_ext_compositor_space_scaler_v1::JayHeadManagerExtCompositorSpaceScalerV1;

pub struct HeadManagerEvent {
    mgr: Rc<JayHeadManagerSessionV1>,
    ty: HeadManagerEventType,
}

pub enum HeadManagerEventType {
    Done,
    TransactionStarted,
    TransactionEnded,
    TransactionResult(Rc<JayHeadTransactionResultV1>),
    Stopped,
}

pub async fn handle_jay_head_manager_done(state: Rc<State>) {
    loop {
        let ev = state.head_managers_async.pop().await;
        let mgr = ev.mgr;
        if mgr.mgr.destroyed.get() {
            continue;
        }
        if mgr.mgr.done_scheduled.take() {
            mgr.mgr.send_done();
        }
        if mgr.common.state.get() == HeadMgrState::Stopped {
            continue;
        }
        match ev.ty {
            HeadManagerEventType::Done => {}
            HeadManagerEventType::TransactionStarted => {
                mgr.send_transaction_started();
            }
            HeadManagerEventType::TransactionEnded => {
                mgr.send_transaction_ended();
            }
            HeadManagerEventType::Stopped => {
                mgr.common.state.set(HeadMgrState::Stopped);
                mgr.send_stopped();
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
    pub id: JayHeadManagerSessionV1Id,
    pub mgr: Rc<JayHeadManagerV1>,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub common: Rc<HeadMgrCommon>,
    pub serial: NumCell<u64>,
    pub core_info_v1: CloneCell<Option<Rc<JayHeadManagerExtCoreInfoV1>>>,
    pub compositor_space_info_v1: CloneCell<Option<Rc<JayHeadManagerExtCompositorSpaceInfoV1>>>,
    pub compositor_space_positioner_v1:
        CloneCell<Option<Rc<JayHeadManagerExtCompositorSpacePositionerV1>>>,
    pub compositor_space_transformer_v1:
        CloneCell<Option<Rc<JayHeadManagerExtCompositorSpaceTransformerV1>>>,
    pub compositor_space_scaler_v1:
        CloneCell<Option<Rc<JayHeadManagerExtCompositorSpaceScalerV1>>>,
    pub physical_display_info_v1: CloneCell<Option<Rc<JayHeadManagerExtPhysicalDisplayInfoV1>>>,
    pub connector_info_v1: CloneCell<Option<Rc<JayHeadManagerExtConnectorInfoV1>>>,
    pub connector_settings_v1: CloneCell<Option<Rc<JayHeadManagerExtConnectorSettingsV1>>>,
    pub heads: CopyHashMap<HeadName, Rc<Head>>,
}

impl JayHeadManagerSessionV1 {
    pub fn announce(self: &Rc<Self>, connector: &ConnectorData) {
        if let Err(e) = self.try_announce(connector) {
            self.client.error(e);
        }
    }

    fn try_announce(self: &Rc<Self>, connector: &ConnectorData) -> Result<(), ClientError> {
        let shared = connector.head_managers.state.clone();
        let common = Rc::new(HeadCommon {
            mgr: self.common.clone(),
            name: connector.head_managers.name,
            removed: Cell::new(false),
            shared: shared.clone(),
            snapshot_state: Default::default(),
            transaction_state: Default::default(),
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
        self.send_head_start(&obj, connector.head_managers.name);
        macro_rules! ext {
            ($($field:ident,)*) => {{
                let head = super::Head {
                    mgr: self.clone(),
                    common: common.clone(),
                    head: obj,
                    $(
                        $field: match self.$field.get() {
                            Some(f) => f.announce(connector, &common)?,
                            _ => None,
                        },
                    )*
                };
                self.send_head_complete();
                let shared = &*shared.borrow();
                $(
                    if let Some(ext) = &head.$field {
                        ext.after_announce_wrapper(shared);
                    }
                )*
                Rc::new(head)
            }};
        }
        let head = ext! {
            core_info_v1,
            compositor_space_info_v1,
            compositor_space_positioner_v1,
            compositor_space_transformer_v1,
            compositor_space_scaler_v1,
            physical_display_info_v1,
            connector_info_v1,
            connector_settings_v1,
        };
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

    pub fn schedule_done(self: &Rc<Self>) {
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

    pub fn send_transaction_started(&self) {
        self.client.event(TransactionStarted { self_id: self.id });
    }

    pub fn send_transaction_ended(&self) {
        self.client.event(TransactionEnded { self_id: self.id });
    }

    pub fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }

    fn send_head_start(&self, head: &JayHeadV1, name: HeadName) {
        self.client.event(HeadStart {
            self_id: self.id,
            head: head.id,
            name: name.0,
        });
    }

    fn send_head_complete(&self) {
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
        result: HeadTransactionResult,
    ) -> Result<(), JayHeadManagerV1Error> {
        self.mgr.done_scheduled.set(true);
        let res = Rc::new(JayHeadTransactionResultV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            result,
            destroyed: Cell::new(false),
        });
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        self.schedule_event(HeadManagerEventType::TransactionResult(res));
        Ok(())
    }

    fn after_transaction_ended(&self) {
        for head in self.heads.lock().values() {
            let _ = head.common.pending.take();
            let tran = &*head.common.transaction_state.borrow();
            let shared = &*head.common.shared.borrow();
            if let Some(i) = &head.core_info_v1 {
                if shared.wl_output != tran.wl_output {
                    i.send_wl_output(shared);
                }
            }
            if let Some(i) = &head.compositor_space_info_v1 {
                if shared.in_compositor_space != tran.in_compositor_space {
                    i.send_inside_outside(shared);
                } else if shared.in_compositor_space {
                    if shared.position != tran.position {
                        i.send_position(shared);
                    }
                    if shared.size != tran.size {
                        i.send_size(shared);
                    }
                    if shared.transform != tran.transform {
                        i.send_transform(shared);
                    }
                    if shared.scale != tran.scale {
                        i.send_scale(shared);
                    }
                }
            }
            if let Some(i) = &head.physical_display_info_v1 {
                let send = match (&shared.monitor_info, &tran.monitor_info) {
                    (Some(s), Some(t)) => !rc_eq(s, t),
                    _ => false,
                };
                if send {
                    i.send_info(shared);
                }
            }
            if let Some(i) = &head.connector_info_v1 {
                if shared.connector_enabled != tran.connector_enabled {
                    i.send_enabled(shared);
                }
                if shared.connected != tran.connected {
                    i.send_connected(shared);
                }
            }
        }
    }
}

impl JayHeadManagerSessionV1RequestHandler for JayHeadManagerSessionV1 {
    type Error = JayHeadManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_stopped()?;
        self.mgr.sessions.fetch_sub(1);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn bind_extension(&self, req: BindExtension<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.get() != HeadMgrState::Init {
            return Err(JayHeadManagerV1Error::AlreadyStarted);
        }
        let Some(ext) = HeadExtension::from_linear(req.name as usize) else {
            return Err(JayHeadManagerV1Error::UnknownExtension(req.name));
        };
        macro_rules! map {
            ($($field:ident = $ext:ident,)*) => {
                match ext {
                    $(
                        HeadExtension::$ext => {
                            if self.$field.is_some() {
                                return Err(JayHeadManagerV1Error::AlreadyBound(req.name));
                            }
                            let version = Version(req.version);
                            with_builtin_macros::with_eager_expansions! {
                                if version > #{concat_idents!(JayHeadManagerExt, $ext)}::VERSION {
                                    return Err(JayHeadManagerV1Error::UnsupportedVersion(req.name, req.version));
                                }
                                let obj = Rc::new(#{concat_idents!(JayHeadManagerExt, $ext)} {
                                    id: req.id.into(),
                                    client: self.client.clone(),
                                    tracker: Default::default(),
                                    version,
                                    common: self.common.clone(),
                                });
                            }
                            track!(self.client, obj);
                            self.client.add_client_obj(&obj)?;
                            self.$field.set(Some(obj));
                        }
                    )*
                }
            };
        }
        map! {
            core_info_v1 = CoreInfoV1,
            compositor_space_info_v1 = CompositorSpaceInfoV1,
            compositor_space_positioner_v1 = CompositorSpacePositionerV1,
            compositor_space_transformer_v1 = CompositorSpaceTransformerV1,
            compositor_space_scaler_v1 = CompositorSpaceScalerV1,
            physical_display_info_v1 = PhysicalDisplayInfoV1,
            connector_info_v1 = ConnectorInfoV1,
            connector_settings_v1 = ConnectorSettingsV1,
        }
        Ok(())
    }

    fn stop(&self, _req: Stop, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.in_transaction.get() {
            return Err(JayHeadManagerV1Error::InTransaction);
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
            return Err(JayHeadManagerV1Error::AlreadyStarted);
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
            return Err(JayHeadManagerV1Error::AlreadyInTransaction);
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
            return Err(JayHeadManagerV1Error::NotInTransaction);
        }
        self.after_transaction_ended();
        slf.schedule_transaction_ended();
        slf.schedule_done();
        Ok(())
    }

    fn apply_changes(&self, req: ApplyChanges, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.common.in_transaction.get() {
            return Err(JayHeadManagerV1Error::NotInTransaction);
        }
        macro_rules! schedule_result {
            ($res:expr) => {
                if !matches!($res, HeadTransactionResult::Success) {
                    self.common.transaction_failed.set(true);
                }
                slf.schedule_transaction_result(req.result, $res)?;
                return Ok(());
            };
        }
        if self.common.transaction_failed.get() {
            schedule_result!(HeadTransactionResult::Failed);
        }
        for head in self.heads.lock().values() {
            let pending = mem::take(&mut *head.common.pending.borrow_mut());
            let snapshot = &*head.common.snapshot_state.borrow();
            let state = &mut *head.common.transaction_state.borrow_mut();
            for op in pending {
                match op {
                    HeadOp::SetPosition(x, y) => {
                        if x < 0 || x > MAX_EXTENTS || y < 0 || y > MAX_EXTENTS {
                            schedule_result!(HeadTransactionResult::PositionOutOfBounds);
                        }
                        state.position = (x, y);
                        if let Some(i) = &head.compositor_space_info_v1 {
                            i.send_position(state);
                        }
                    }
                    HeadOp::SetConnectorEnabled(enabled) => {
                        if state.connector_enabled == enabled {
                            continue;
                        }
                        state.connector_enabled = enabled;
                        if enabled {
                            state.in_compositor_space = snapshot.in_compositor_space;
                            state.connected = snapshot.connected;
                            state.monitor_info = snapshot.monitor_info.clone();
                            state.wl_output = snapshot.wl_output;
                        } else {
                            state.in_compositor_space = false;
                            state.connected = false;
                            state.monitor_info = None;
                            state.wl_output = None;
                        }
                        if let Some(i) = &head.compositor_space_info_v1 {
                            i.send_inside_outside(state);
                        }
                        if let Some(i) = &head.connector_info_v1 {
                            i.send_enabled(state);
                            i.send_connected(state);
                        }
                        if let Some(i) = &head.core_info_v1 {
                            i.send_wl_output(state);
                        }
                        if let Some(i) = &head.physical_display_info_v1 {
                            i.send_info(state);
                        }
                    }
                    HeadOp::SetTransform(t) => {
                        state.transform = t;
                        state.size = OutputNode::calculate_extents_(
                            state.mode,
                            state.transform,
                            state.scale,
                            state.position,
                        )
                        .size();
                        if let Some(i) = &head.compositor_space_info_v1 {
                            i.send_transform(state);
                            i.send_size(state);
                        }
                    }
                    HeadOp::SetScale(s) => {
                        if s < MIN_SCALE || s > MAX_SCALE {
                            schedule_result!(HeadTransactionResult::ScaleOutOfBounds);
                        }
                        state.scale = s;
                        state.size = OutputNode::calculate_extents_(
                            state.mode,
                            state.transform,
                            state.scale,
                            state.position,
                        )
                        .size();
                        if let Some(i) = &head.compositor_space_info_v1 {
                            i.send_scale(state);
                            i.send_size(state);
                        }
                    }
                }
            }
        }
        schedule_result!(HeadTransactionResult::Success);
    }

    fn test_transaction(&self, _req: TestTransaction, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.in_transaction.replace(false) {
            return Err(JayHeadManagerV1Error::NotInTransaction);
        }
        todo!()
    }

    fn commit_transaction(
        &self,
        _req: CommitTransaction,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.common.in_transaction.replace(false) {
            return Err(JayHeadManagerV1Error::NotInTransaction);
        }
        todo!()
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

dedicated_add_obj!(
    JayHeadManagerSessionV1,
    JayHeadManagerSessionV1Id,
    jay_head_managers
);

#[derive(Debug, Error)]
pub enum JayHeadManagerV1Error {
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
    #[error("The extension with name {} does not support version {}", .0, .1)]
    UnsupportedVersion(u32, u32),
    #[error("There already is an active transaction")]
    AlreadyInTransaction,
    #[error("There is no active transaction")]
    NotInTransaction,
    #[error("There is an active transaction")]
    InTransaction,
}
efrom!(JayHeadManagerV1Error, ClientError);
