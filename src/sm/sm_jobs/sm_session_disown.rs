use {
    crate::{
        sm::{
            SessionId, SessionManager,
            sm_jobs::{SmDbStateHolder, sm_common::CreateDbStateError},
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork},
        utils::errorfmt::ErrorFmt,
    },
    std::{
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
};

pub struct SessionDisownJob {
    pub work: SessionDisownWork,
    manager: Weak<SessionManager>,
}

pub struct SessionDisownWork {
    pub session_id: SessionId,
    db_state: Arc<SmDbStateHolder>,
}

#[derive(Debug, Error)]
enum SessionDisownError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
}

impl SessionDisownJob {
    pub fn new(manager: &Rc<SessionManager>) -> Self {
        Self {
            work: SessionDisownWork {
                session_id: SessionId(-1),
                db_state: manager.db_state.clone(),
            },
            manager: Rc::downgrade(manager),
        }
    }
}

impl SqliteJob for SessionDisownJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        if let Some(manager) = self.manager.upgrade() {
            manager.session_disown_jobs.push(self);
        }
    }
}

impl SqliteWork for SessionDisownWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        if let Err(e) = self.try_run(ctx) {
            log::error!("Disown failed: {}", ErrorFmt(e));
        }
    }
}

impl SessionDisownWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), SessionDisownError> {
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_write()?;
        let stmt = s.s.session_disown.activate();
        stmt.bind_i64(1, self.session_id.0)?;
        stmt.bind_user_id(2, ctx.user_id)?;
        stmt.exec()?;
        let stmt = s.s.toplevel_disown_all.activate();
        stmt.bind_i64(1, self.session_id.0)?;
        stmt.bind_user_id(2, ctx.user_id)?;
        stmt.exec()?;
        tx.commit()?;
        Ok(())
    }
}
