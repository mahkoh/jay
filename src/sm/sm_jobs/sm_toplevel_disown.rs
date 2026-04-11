use {
    crate::{
        sm::{
            SessionManager, ToplevelSessionId,
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

pub struct ToplevelDisownJob {
    pub work: ToplevelDisownWork,
    manager: Weak<SessionManager>,
}

pub struct ToplevelDisownWork {
    pub toplevel_id: ToplevelSessionId,
    db_state: Arc<SmDbStateHolder>,
}

#[derive(Debug, Error)]
enum ToplevelDisownError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
}

impl ToplevelDisownJob {
    pub fn new(manager: &Rc<SessionManager>) -> Self {
        Self {
            work: ToplevelDisownWork {
                toplevel_id: ToplevelSessionId(-1),
                db_state: manager.db_state.clone(),
            },
            manager: Rc::downgrade(manager),
        }
    }
}

impl SqliteJob for ToplevelDisownJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        if let Some(manager) = self.manager.upgrade() {
            manager.toplevel_disown_jobs.push(self);
        }
    }
}

impl SqliteWork for ToplevelDisownWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        if let Err(e) = self.try_run(ctx) {
            log::error!("Disown failed: {}", ErrorFmt(e));
        }
    }
}

impl ToplevelDisownWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), ToplevelDisownError> {
        db_state!(s, self, ctx);
        let stmt = s.s.toplevel_disown_one.activate();
        stmt.bind_i64(1, self.toplevel_id.0)?;
        stmt.bind_user_id(2, ctx.user_id)?;
        stmt.exec()?;
        Ok(())
    }
}
