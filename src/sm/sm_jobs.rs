pub use crate::sm::sm_jobs::sm_common::SmDbStateHolder;
use {
    crate::{
        sm::{Session, SessionManager},
        sqlite::{SqliteJob, SqliteUsage},
        utils::{cell_ext::CellExt, stack::Stack},
    },
    std::{
        cell::Cell,
        rc::{Rc, Weak},
        sync::Arc,
    },
};

#[macro_use]
mod sm_common;
pub mod sm_session_acquire;
pub mod sm_session_del;
pub mod sm_session_disown;
pub mod sm_session_list;
pub mod sm_toplevel_acquire;
pub mod sm_toplevel_del;
pub mod sm_toplevel_disown;
pub mod sm_toplevel_rename;
pub mod sm_toplevel_roundtrip;
pub mod sm_toplevel_update;

pub trait SmJob: SqliteJob + Sized + 'static {
    type Cb: ?Sized;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self;

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>>;
}

pub struct SmPending<Job: SmJob> {
    job: Cell<Option<Box<Job>>>,
    cb: Cell<Option<Rc<Job::Cb>>>,
}

impl<Job: SmJob> SmPending<Job> {
    pub fn new(db_state: &Arc<SmDbStateHolder>) -> Rc<Self> {
        Rc::new_cyclic(|slf| Self {
            job: Cell::new(Some(Box::new(Job::new(db_state.clone(), slf.clone())))),
            cb: Default::default(),
        })
    }
}

pub struct SmScheduled<Job>
where
    Job: SmJob,
{
    manager: Rc<SessionManager>,
    pending: Rc<SmPending<Job>>,
}

impl<Job: SmJob> Drop for SmScheduled<Job> {
    fn drop(&mut self) {
        self.pending.cb.take();
        if self.pending.job.is_none() {
            return;
        }
        Job::stack(&self.manager).push(self.pending.clone());
    }
}

impl Session {
    pub(super) fn add_job<Job: SmJob>(
        self: &Rc<Self>,
        cb: &Rc<Job::Cb>,
        configure: impl FnOnce(&mut Job),
    ) -> SmScheduled<Job> {
        self.manager.add_job(cb, self.usage(), configure)
    }
}

impl SessionManager {
    pub(super) fn add_job<Job: SmJob>(
        self: &Rc<Self>,
        cb: &Rc<Job::Cb>,
        usage: Option<SqliteUsage>,
        configure: impl FnOnce(&mut Job),
    ) -> SmScheduled<Job> {
        let pending = Job::stack(self)
            .pop()
            .unwrap_or_else(|| SmPending::new(&self.db_state));
        let mut job = pending.job.take().unwrap();
        configure(&mut job);
        self.sqlite.add_job(usage, job);
        pending.cb.set(Some(cb.clone()));
        SmScheduled {
            manager: self.clone(),
            pending,
        }
    }
}
