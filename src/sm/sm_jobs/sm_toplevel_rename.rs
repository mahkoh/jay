use {
    crate::{
        sm::{
            SessionId, SessionManager, ToplevelSession, ToplevelSessionId, ToplevelSessionName,
            sm_jobs::{
                SmDbStateHolder, SmJob, SmPending,
                sm_common::{CreateDbStateError, SessionOwnerError},
            },
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork},
        utils::stack::Stack,
    },
    std::{
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
};

pub struct ToplevelRenameJob {
    pub work: ToplevelRenameWork,
    pending: Weak<SmPending<Self>>,
}

pub struct ToplevelRenameWork {
    pub session_id: SessionId,
    pub id: ToplevelSessionId,
    pub name: ToplevelSessionName,
    pub name_text: String,
    result: Option<Result<(), ToplevelRenameError>>,
    db_state: Arc<SmDbStateHolder>,
}

#[derive(Debug, Error)]
enum ToplevelRenameError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error(transparent)]
    Owner(#[from] SessionOwnerError),
}

impl SmJob for ToplevelRenameJob {
    type Cb = ToplevelSession;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            work: ToplevelRenameWork {
                session_id: SessionId(-1),
                id: ToplevelSessionId(-1),
                name: ToplevelSessionName(Default::default()),
                name_text: Default::default(),
                result: Default::default(),
                db_state,
            },
            pending,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.toplevel_rename_jobs
    }
}

impl SqliteJob for ToplevelRenameJob {
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
        let Err(err) = res else {
            cb.schedule_job(true);
            return;
        };
        if let ToplevelRenameError::Owner(SessionOwnerError::NotOwned) = err {
            cb.session.replaced_external();
            return;
        }
        cb.fatal(err);
    }
}

impl SqliteWork for ToplevelRenameWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        self.result = Some(self.try_run(ctx));
    }
}

impl ToplevelRenameWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), ToplevelRenameError> {
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_write()?;
        assert_session_owner!(s, self.session_id, ctx.user_id)?;
        let stmt = s.s.toplevel_rename.activate();
        stmt.bind_blob(1, &self.name.0)?;
        stmt.bind_blob(2, self.name_text.as_bytes())?;
        stmt.bind_i64(3, self.id.0)?;
        stmt.exec()?;
        tx.commit()?;
        Ok(())
    }
}
