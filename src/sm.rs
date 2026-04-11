use {
    crate::{
        client::Client,
        ifs::wl_output::OutputIdHash,
        rect::Rect,
        sm::{
            sm_jobs::{
                SmDbStateHolder, SmPending, SmScheduled,
                sm_session_acquire::SessionAcquireJob,
                sm_session_del::SessionDelJob,
                sm_session_disown::SessionDisownJob,
                sm_session_list::{SessionListJob, SessionListRequest},
                sm_toplevel_acquire::ToplevelAcquireJob,
                sm_toplevel_del::ToplevelDelJob,
                sm_toplevel_disown::ToplevelDisownJob,
                sm_toplevel_rename::ToplevelRenameJob,
                sm_toplevel_roundtrip::ToplevelRoundtripJob,
                sm_toplevel_update::ToplevelUpdateJob,
            },
            sm_wire::{
                sm_wire_session::{SmSessionIn, SmSessionInUseData, SmSessionOut},
                sm_wire_toplevel::{SmToplevelIn, SmToplevelOut},
            },
        },
        sqlite::{Sqlite, SqliteError, SqliteUsage},
        state::State,
        tree::{Node, OutputNode, ToplevelData, WorkspaceHash, WorkspaceNode},
        utils::{
            asyncevent::AsyncEvent,
            cell_ext::CellExt,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            event_listener::{EventListener, EventSource},
            hash_map_ext::HashMapExt,
            send_sync_rc::SendSyncRc,
            stack::Stack,
            thread_id::ThreadId,
        },
    },
    std::{
        cell::{Cell, RefCell},
        error::Error,
        rc::{Rc, Weak},
        sync::Arc,
        time::{Duration, SystemTime},
    },
    thiserror::Error,
};

mod sm_jobs;
mod sm_wire;

#[derive(Debug, Error)]
pub enum SessionManagementError {
    #[error("The name is already in use in this session")]
    NameInUse,
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
}

pub struct SessionManager {
    sqlite: Rc<Sqlite>,
    thread_id: ThreadId,
    db_state: Arc<SmDbStateHolder>,
    sessions: CopyHashMap<SessionNameHash, Weak<Session>>,
    updated_toplevels: EventSource<ToplevelSession>,
    session_acquire_jobs: Stack<Rc<SmPending<SessionAcquireJob>>>,
    session_list_jobs: Stack<Rc<SmPending<SessionListJob>>>,
    session_del_jobs: Stack<Box<SessionDelJob>>,
    session_disown_jobs: Stack<Box<SessionDisownJob>>,
    toplevel_acquire_jobs: Stack<Rc<SmPending<ToplevelAcquireJob>>>,
    toplevel_del_jobs: Stack<Box<ToplevelDelJob>>,
    toplevel_rename_jobs: Stack<Rc<SmPending<ToplevelRenameJob>>>,
    toplevel_update_jobs: Stack<Rc<SmPending<ToplevelUpdateJob>>>,
    toplevel_disown_jobs: Stack<Box<ToplevelDisownJob>>,
    toplevel_roundtrip_jobs: Stack<Rc<SmPending<ToplevelRoundtripJob>>>,
}

pub enum SessionGetStatus {
    Created,
    Restored,
}

pub struct Session {
    manager: Rc<SessionManager>,
    name: SessionName,
    hash: SessionNameHash,
    id: Cell<Option<SessionId>>,
    owner: CloneCell<Option<Rc<dyn SessionOwner>>>,
    toplevels: CopyHashMap<ToplevelSessionName, Weak<ToplevelSession>>,
    job: Cell<Option<SessionJob>>,
    restore: Cell<bool>,
    reason: Cell<SessionReason>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SessionReason {
    Launch,
    Recover,
    SessionRestore,
}

pub trait SessionOwner {
    fn client(&self) -> Option<&Rc<Client>>;
    fn loaded(&self, status: SessionGetStatus);
    fn error(&self, e: &dyn Error);
    fn disown_from_peer(&self, replaced: bool);
}

enum SessionJob {
    Acquire(#[expect(dead_code)] SmScheduled<SessionAcquireJob>),
}

pub struct ToplevelSession {
    pub session: Rc<Session>,
    name: Cell<ToplevelSessionName>,
    name_text: RefCell<String>,
    renamed: Cell<bool>,
    changed: Cell<bool>,
    listener_attached: Cell<bool>,
    changed_listener: EventListener<ToplevelSession>,
    restore: Cell<bool>,
    id: Cell<Option<ToplevelSessionId>>,
    owner: CloneCell<Option<Rc<dyn ToplevelSessionOwner>>>,
    pub state: ToplevelSessionState,
    job: Cell<Option<ToplevelJob>>,
}

pub trait ToplevelSessionOwner {
    fn disown_from_peer(self: Rc<Self>);
    fn loaded(&self, status: SessionGetStatus);
}

enum ToplevelJob {
    Roundtrip(#[expect(dead_code)] SmScheduled<ToplevelRoundtripJob>),
    Acquire(#[expect(dead_code)] SmScheduled<ToplevelAcquireJob>),
    Rename(#[expect(dead_code)] SmScheduled<ToplevelRenameJob>),
    Update(#[expect(dead_code)] SmScheduled<ToplevelUpdateJob>),
}

pub struct SessionListToplevel {
    #[expect(dead_code)]
    pub name: ToplevelSessionName,
    pub name_text: String,
    #[expect(dead_code)]
    pub ctime: SystemTime,
    #[expect(dead_code)]
    pub atime: SystemTime,
    #[expect(dead_code)]
    pub data: SmToplevelOut,
}

pub struct SessionListSession {
    pub name: SessionName,
    #[expect(dead_code)]
    pub ctime: SystemTime,
    #[expect(dead_code)]
    pub atime: SystemTime,
    #[expect(dead_code)]
    pub data: SmSessionOut,
    pub toplevels: Vec<SessionListToplevel>,
}

pub struct SessionList {
    #[expect(dead_code)]
    pub sessions: Vec<SessionListSession>,
}

pub struct SessionListScheduled {
    _s: SmScheduled<SessionListJob>,
}

#[derive(Default, Clone)]
pub struct ToplevelSessionState {
    pub output: Cell<Option<OutputIdHash>>,
    pub floating_pos: Cell<Option<Rect>>,
    pub workspace: CloneCell<Option<Rc<String>>>,
    workspace_hash: Cell<Option<WorkspaceHash>>,
    pub fullscreen: Cell<bool>,
}

opaque!(SessionName, session_name);

hash_type!(SessionNameHash);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct SessionId(i64);

hash_type!(ToplevelSessionName);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ToplevelSessionId(i64);

impl SessionGetStatus {
    fn from_restored(restored: bool) -> Self {
        match restored {
            true => Self::Restored,
            false => Self::Created,
        }
    }
}

impl SessionManager {
    pub fn new(sqlite: &Rc<Sqlite>) -> Self {
        Self {
            sqlite: sqlite.clone(),
            thread_id: Default::default(),
            db_state: Arc::new(SmDbStateHolder::new(sqlite)),
            sessions: Default::default(),
            updated_toplevels: Default::default(),
            session_acquire_jobs: Default::default(),
            session_del_jobs: Default::default(),
            session_disown_jobs: Default::default(),
            session_list_jobs: Default::default(),
            toplevel_acquire_jobs: Default::default(),
            toplevel_del_jobs: Default::default(),
            toplevel_rename_jobs: Default::default(),
            toplevel_update_jobs: Default::default(),
            toplevel_disown_jobs: Default::default(),
            toplevel_roundtrip_jobs: Default::default(),
        }
    }

    pub fn flush_all(&self) {
        for session in self.sessions.lock().values() {
            if let Some(session) = session.upgrade() {
                session.schedule_jobs();
            }
        }
    }

    pub fn clear(&self) {
        for session in self.sessions.lock().drain_values() {
            if let Some(session) = session.upgrade() {
                session.job.take();
                if let Some(owner) = session.owner.take() {
                    owner.disown_from_peer(false);
                }
                for ts in session.toplevels.lock().drain_values() {
                    if let Some(ts) = ts.upgrade() {
                        ts.job.take();
                        if let Some(owner) = ts.owner.take() {
                            owner.disown_from_peer();
                        }
                    }
                }
            }
        }
    }

    pub fn get(
        self: &Rc<Self>,
        name: SessionName,
        restore: bool,
        reason: SessionReason,
        owner: Rc<dyn SessionOwner>,
    ) -> (Rc<Session>, Option<SessionGetStatus>) {
        let hash = SessionNameHash::hash(name.0.as_bytes());
        let session = match self.sessions.get(&hash).and_then(|s| s.upgrade()) {
            Some(s) => s,
            None => {
                let s = Rc::new(Session {
                    manager: self.clone(),
                    name,
                    hash,
                    id: Default::default(),
                    owner: Default::default(),
                    toplevels: Default::default(),
                    job: Default::default(),
                    restore: Default::default(),
                    reason: Cell::new(reason),
                });
                self.sessions.set(hash, Rc::downgrade(&s));
                s
            }
        };
        session.disown_to_peer(true);
        session.restore.set(restore);
        session.reason.set(reason);
        session.owner.set(Some(owner));
        session.schedule_jobs();
        let status = session
            .id
            .get()
            .is_some()
            .then_some(SessionGetStatus::from_restored(restore));
        (session, status)
    }

    #[expect(dead_code)]
    pub fn list(
        self: &Rc<Self>,
        cb: impl FnOnce(Result<SessionList, Box<dyn Error>>) + 'static,
    ) -> SessionListScheduled {
        let req = Rc::new(SessionListRequest {
            cb: Cell::new(Some(Box::new(cb))),
        });
        let s = self.add_job(&req, None, |_: &mut SessionListJob| {});
        SessionListScheduled { _s: s }
    }
}

impl Session {
    pub fn reason(&self) -> SessionReason {
        self.reason.get()
    }

    fn usage(&self) -> Option<SqliteUsage> {
        let owner = self.owner.get()?;
        let client = owner.client()?;
        match client.sqlite_accounting.reserve() {
            Ok(u) => Some(u),
            Err(e) => {
                client.error(e);
                None
            }
        }
    }

    fn replaced_external(&self) {
        self.disown_to_peer(true);
        self.manager.sessions.remove(&self.hash);
        self.job.set(None);
        for ts in self.toplevels.lock().drain_values() {
            if let Some(ts) = ts.upgrade() {
                ts.job.take();
            }
        }
    }

    fn disown_(&self, to_peer: bool, replaced: bool) {
        if let Some(owner) = self.owner.take() {
            if to_peer {
                owner.disown_from_peer(replaced);
            }
        }
        for ts in self.toplevels.lock().values() {
            if let Some(ts) = ts.upgrade() {
                ts.disown_to_peer();
            }
        }
    }

    pub fn disown_from_peer(&self) {
        self.disown_(false, false);
    }

    pub fn disown_to_peer(&self, replaced: bool) {
        self.disown_(true, replaced);
    }

    fn schedule_jobs(self: &Rc<Self>) {
        if self.job.is_some() {
            return;
        }
        if self.id.is_none() {
            let owner = self.owner.get();
            let client = owner.as_ref().and_then(|o| o.client());
            let tid = self.manager.thread_id;
            let job = self.add_job(&self, |job: &mut SessionAcquireJob| {
                job.work.session = self.name;
                job.work.restore = self.restore.get();
                job.work.data = SmSessionIn {
                    last_acquire: SmSessionInUseData {
                        exe: client.map(|c| SendSyncRc::new(tid, &c.pid_info.exe)),
                    },
                };
            });
            self.job.set(Some(SessionJob::Acquire(job)));
            return;
        }
        for tl in self.toplevels.lock().values() {
            if let Some(tl) = tl.upgrade() {
                tl.schedule_job(true);
            }
        }
    }

    fn fatal(&self, e: impl Error) {
        if let Some(owner) = self.owner.get() {
            owner.error(&e);
        } else {
            log::error!(
                "A fatal error occurred in session but there is no owner to report to: {}",
                ErrorFmt(e),
            );
        }
        self.disown_to_peer(false);
    }

    pub fn get(
        self: &Rc<Self>,
        name_text: &str,
        owner: Rc<dyn ToplevelSessionOwner>,
        restore: bool,
    ) -> Result<(Rc<ToplevelSession>, Option<SessionGetStatus>), SessionManagementError> {
        let name = self.name.toplevel(name_text);
        let ts = match self.toplevels.get(&name).and_then(|s| s.upgrade()) {
            Some(s) => {
                if !restore || s.owner.is_some() {
                    return Err(SessionManagementError::NameInUse);
                }
                s
            }
            None => {
                let s = Rc::new_cyclic(|slf| ToplevelSession {
                    session: self.clone(),
                    name: Cell::new(name),
                    name_text: RefCell::new(name_text.to_string()),
                    renamed: Default::default(),
                    changed: Default::default(),
                    listener_attached: Default::default(),
                    changed_listener: EventListener::new(slf.clone()),
                    restore: Default::default(),
                    id: Default::default(),
                    owner: Default::default(),
                    state: Default::default(),
                    job: Default::default(),
                });
                self.toplevels.set(name, Rc::downgrade(&s));
                s
            }
        };
        ts.restore.set(restore);
        ts.owner.set(Some(owner));
        ts.schedule_job(true);
        let status = ts
            .id
            .get()
            .is_some()
            .then_some(SessionGetStatus::from_restored(restore));
        Ok((ts, status))
    }

    pub fn remove_toplevel(&self, name: &str) {
        let name = self.name.toplevel(name);
        let mut job = self
            .manager
            .toplevel_del_jobs
            .pop()
            .unwrap_or_else(|| Box::new(ToplevelDelJob::new(&self.manager)));
        job.work.session = self.name;
        job.work.toplevel = name;
        self.manager.sqlite.add_job(None, job);
        let ts = self
            .toplevels
            .remove(&name)
            .as_ref()
            .and_then(Weak::upgrade);
        let Some(ts) = ts else {
            return;
        };
        ts.job.take();
        ts.disown_to_peer();
    }

    pub fn remove(&self) {
        let mut job = self
            .manager
            .session_del_jobs
            .pop()
            .unwrap_or_else(|| Box::new(SessionDelJob::new(&self.manager)));
        job.work.session = self.name;
        self.manager.sqlite.add_job(None, job);
        self.disown_from_peer();
        self.job.take();
        self.manager.sessions.remove(&self.hash);
    }
}

impl ToplevelSession {
    fn state_changed(self: &Rc<Self>) {
        self.changed.set(true);
        self.schedule_job(false);
    }

    pub fn set_workspace(self: &Rc<Self>, ws: &WorkspaceNode, data: &ToplevelData) {
        let hash = (!ws.is_dummy).then_some(ws.hash);
        if hash == self.state.workspace_hash.get() {
            return;
        }
        self.state.workspace_hash.set(hash);
        self.state
            .workspace
            .set((!ws.is_dummy).then_some(ws.name.clone()));
        self.state_changed();
        self.set_output(&ws.output.get(), data);
    }

    pub fn set_output(self: &Rc<Self>, on: &OutputNode, data: &ToplevelData) {
        let hash = (!on.is_dummy).then_some(on.global.output_id.hash);
        if hash == self.state.output.get() {
            return;
        }
        self.state.output.set(hash);
        self.state_changed();
        self.set_float_pos(data);
    }

    pub fn set_float_pos(self: &Rc<Self>, data: &ToplevelData) {
        let rect = if data.parent_is_float.get() {
            let on = data.output();
            if on.is_dummy {
                None
            } else {
                let on = on.node_absolute_position();
                let rect = data.desired_extents.get().move_(-on.x1(), -on.y1());
                Some(rect)
            }
        } else {
            None
        };
        if rect == self.state.floating_pos.get() {
            return;
        }
        self.state.floating_pos.set(rect);
        self.state_changed();
    }

    pub fn set_fullscreen(self: &Rc<Self>, fullscreen: bool) {
        if fullscreen == self.state.fullscreen.get() {
            return;
        }
        self.state.fullscreen.set(fullscreen);
        self.state_changed();
    }

    fn schedule_job(self: &Rc<Self>, allow_changed: bool) {
        if self.job.is_some() {
            return;
        }
        let Some(session_id) = self.session.id.get() else {
            let job = self
                .session
                .add_job(self, |_job: &mut ToplevelRoundtripJob| {});
            self.job.set(Some(ToplevelJob::Roundtrip(job)));
            return;
        };
        let id = match self.id.get() {
            Some(id) => id,
            None => {
                let job = self.session.add_job(self, |job: &mut ToplevelAcquireJob| {
                    job.work.name_text = self.name_text.borrow().clone();
                    job.work.name = self.name.get();
                    job.work.session_id = session_id;
                    job.work.restore = self.restore.get();
                });
                self.job.set(Some(ToplevelJob::Acquire(job)));
                return;
            }
        };
        if self.renamed.take() {
            let job = self.session.add_job(self, |job: &mut ToplevelRenameJob| {
                job.work.name_text = self.name_text.borrow().clone();
                job.work.name = self.name.get();
                job.work.session_id = session_id;
                job.work.id = id;
            });
            self.job.set(Some(ToplevelJob::Rename(job)));
            return;
        }
        if self.changed.get() {
            if allow_changed {
                if self.listener_attached.replace(false) {
                    self.changed_listener.detach();
                }
            } else {
                if !self.listener_attached.replace(true) {
                    self.changed_listener
                        .attach(&self.session.manager.updated_toplevels);
                }
                return;
            }
            self.changed.take();
            let s = &self.state;
            let tid = self.session.manager.thread_id;
            let job = self.session.add_job(self, |job: &mut ToplevelUpdateJob| {
                job.work.session_id = session_id;
                job.work.id = id;
                job.work.data = SmToplevelIn {
                    output: s.output.get(),
                    workspace: s.workspace.get().map(|v| SendSyncRc::new(tid, &v)),
                    floating_pos: s.floating_pos.get(),
                    fullscreen: s.fullscreen.get(),
                };
            });
            self.job.set(Some(ToplevelJob::Update(job)));
            return;
        }
    }

    fn disown_(self: &Rc<Self>, to_peer: bool) {
        if self.id.is_some() {
            self.schedule_job(true);
        }
        if let Some(owner) = self.owner.take() {
            if to_peer {
                owner.disown_from_peer();
            }
        }
    }

    pub fn disown_to_peer(self: &Rc<Self>) {
        self.disown_(true);
    }

    pub fn disown_from_peer(self: &Rc<Self>) {
        self.disown_(false);
    }

    fn fatal(&self, msg: impl Error) {
        self.session.fatal(msg);
    }

    pub fn rename(self: &Rc<Self>, new_text: &str) -> Result<(), SessionManagementError> {
        let old = self.name.get();
        let new = self.session.name.toplevel(new_text);
        if old == new {
            return Ok(());
        }
        if self.session.toplevels.contains(&new) {
            return Err(SessionManagementError::NameInUse);
        }
        self.session.toplevels.remove(&old);
        self.session.toplevels.set(new, Rc::downgrade(self));
        self.name.set(new);
        {
            let text = &mut *self.name_text.borrow_mut();
            text.clear();
            text.push_str(new_text);
        }
        self.renamed.set(true);
        self.schedule_job(true);
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let m = &self.manager;
        m.sessions.remove(&self.hash);
        if let Some(id) = self.id.get() {
            let mut job = m
                .session_disown_jobs
                .pop()
                .unwrap_or_else(|| Box::new(SessionDisownJob::new(m)));
            job.work.session_id = id;
            m.sqlite.add_job(None, job);
        }
    }
}

impl Drop for ToplevelSession {
    fn drop(&mut self) {
        self.session.toplevels.remove(&self.name.get());
        if let Some(id) = self.id.get() {
            let m = &self.session.manager;
            let mut job = m
                .toplevel_disown_jobs
                .pop()
                .unwrap_or_else(|| Box::new(ToplevelDisownJob::new(m)));
            job.work.toplevel_id = id;
            m.sqlite.add_job(None, job);
        }
    }
}

pub async fn flush_toplevel_sessions(state: Rc<State>) {
    let Some(sm) = &state.sm else {
        return;
    };
    let on_attached = Rc::new(AsyncEvent::default());
    loop {
        if sm.updated_toplevels.is_empty() {
            sm.updated_toplevels.on_attach({
                let on_attached = on_attached.clone();
                Box::new(move || on_attached.trigger())
            });
            on_attached.triggered().await;
            continue;
        }
        for tl in sm.updated_toplevels.iter() {
            tl.schedule_job(true);
        }
        const FIVE_SECONDS_NS: u64 = Duration::from_secs(5).as_nanos() as u64;
        let res = state.ring.timeout(state.now_nsec() + FIVE_SECONDS_NS).await;
        if let Err(e) = res {
            log::error!("Could not wait for timeout to expire: {}", ErrorFmt(e));
            return;
        }
    }
}

impl SessionName {
    pub fn toplevel(&self, name: &str) -> ToplevelSessionName {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&[0]);
        hasher.update(self.0.as_bytes());
        hasher.update(name.as_bytes());
        ToplevelSessionName(*hasher.finalize().as_bytes())
    }
}
