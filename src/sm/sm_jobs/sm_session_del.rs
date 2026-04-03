use {
    crate::{
        sm::{
            SessionManager, SessionName,
            sm_jobs::{SmDbStateHolder, sm_common::CreateDbStateError},
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork},
        utils::{errorfmt::ErrorFmt, opaque::opaque},
    },
    std::{
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
};

pub struct SessionDelJob {
    pub work: SessionDelWork,
    manager: Weak<SessionManager>,
}

pub struct SessionDelWork {
    pub session: SessionName,
    db_state: Arc<SmDbStateHolder>,
}

#[derive(Debug, Error)]
enum SessionDelError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
}

impl SessionDelJob {
    pub fn new(manager: &Rc<SessionManager>) -> Self {
        Self {
            work: SessionDelWork {
                session: SessionName(opaque()),
                db_state: manager.db_state.clone(),
            },
            manager: Rc::downgrade(manager),
        }
    }
}

impl SqliteJob for SessionDelJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        if let Some(manager) = self.manager.upgrade() {
            manager.session_del_jobs.push(self);
        }
    }
}

impl SqliteWork for SessionDelWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        if let Err(e) = self.try_run(ctx) {
            log::error!("Delete session failed: {}", ErrorFmt(e));
        }
    }
}

impl SessionDelWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), SessionDelError> {
        db_state!(s, self, ctx);
        let stmt = s.s.session_del.activate();
        stmt.bind_blob(1, self.session.0.as_bytes())?;
        stmt.bind_user_id(2, ctx.user_id)?;
        stmt.exec()?;
        Ok(())
    }
}
