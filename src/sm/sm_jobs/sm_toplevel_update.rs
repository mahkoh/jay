use crate::sm::SessionId;
use crate::sm::SessionManager;
use crate::sm::ToplevelSession;
use crate::sm::ToplevelSessionId;
use crate::sm::sm_jobs::SmDbStateHolder;
use crate::sm::sm_jobs::SmJob;
use crate::sm::sm_jobs::SmPending;
use crate::sm::sm_jobs::sm_common::CreateDbStateError;
use crate::sm::sm_jobs::sm_common::SessionOwnerError;
use crate::sm::sm_wire::sm_wire_toplevel::SmToplevelIn;
use crate::sm::sm_wire::sm_wire_toplevel::serialize_toplevel;
use crate::sqlite::SqliteCtx;
use crate::sqlite::SqliteError;
use crate::sqlite::SqliteJob;
use crate::sqlite::SqliteWork;
use crate::utils::stack::Stack;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use thiserror::Error;

pub struct ToplevelUpdateJob {
    pub work: ToplevelUpdateWork,
    pending: Weak<SmPending<Self>>,
}

pub struct ToplevelUpdateWork {
    pub session_id: SessionId,
    pub id: ToplevelSessionId,
    pub data: SmToplevelIn,
    db_state: Arc<SmDbStateHolder>,
    stash: Vec<u8>,
    result: Option<Result<(), ToplevelUpdateError>>,
}

#[derive(Debug, Error)]
pub enum ToplevelUpdateError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error(transparent)]
    Owner(#[from] SessionOwnerError),
}

impl SmJob for ToplevelUpdateJob {
    type Cb = ToplevelSession;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            work: ToplevelUpdateWork {
                session_id: SessionId(-1),
                id: ToplevelSessionId(-1),
                data: Default::default(),
                db_state,
                stash: Default::default(),
                result: Default::default(),
            },
            pending,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.toplevel_update_jobs
    }
}

impl SqliteJob for ToplevelUpdateJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        let res = self.work.result.take().unwrap();
        let Some(pending) = self.pending.upgrade() else {
            return;
        };
        pending.job.set(Some(self));
        let Some(cb) = pending.cb.take() else {
            return;
        };
        cb.job.take();
        let Err(e) = res else {
            cb.schedule_job(false);
            return;
        };
        if let ToplevelUpdateError::Owner(SessionOwnerError::NotOwned) = e {
            cb.session.replaced_external();
            return;
        }
        cb.fatal(e);
    }
}

impl SqliteWork for ToplevelUpdateWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        self.result = Some(self.try_run(ctx));
    }
}

impl ToplevelUpdateWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), ToplevelUpdateError> {
        serialize_toplevel(&mut self.stash, &self.data);
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_write()?;
        assert_session_owner!(s, self.session_id, ctx.user_id)?;
        let stmt = s.s.toplevel_store.activate();
        stmt.bind_blob(1, &self.stash)?;
        stmt.bind_i64(2, self.id.0)?;
        stmt.exec()?;
        tx.commit()?;
        Ok(())
    }
}
