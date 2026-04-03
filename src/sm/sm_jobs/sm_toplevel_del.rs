use {
    crate::{
        sm::{
            SessionManager, SessionName, ToplevelSessionName,
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

pub struct ToplevelDelJob {
    pub work: ToplevelDelWork,
    manager: Weak<SessionManager>,
}

pub struct ToplevelDelWork {
    pub session: SessionName,
    pub toplevel: ToplevelSessionName,
    db_state: Arc<SmDbStateHolder>,
}

#[derive(Debug, Error)]
enum ToplevelDelError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
}

impl ToplevelDelJob {
    pub fn new(manager: &Rc<SessionManager>) -> Self {
        Self {
            work: ToplevelDelWork {
                session: SessionName(opaque()),
                toplevel: ToplevelSessionName(Default::default()),
                db_state: manager.db_state.clone(),
            },
            manager: Rc::downgrade(manager),
        }
    }
}

impl SqliteJob for ToplevelDelJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        if let Some(manager) = self.manager.upgrade() {
            manager.toplevel_del_jobs.push(self);
        }
    }
}

impl SqliteWork for ToplevelDelWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        if let Err(e) = self.try_run(ctx) {
            log::error!("Delete toplevel failed: {}", ErrorFmt(e));
        }
    }
}

impl ToplevelDelWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<(), ToplevelDelError> {
        db_state!(s, self, ctx);
        let stmt = s.s.toplevel_del.activate();
        stmt.bind_blob(1, &self.toplevel.0)?;
        stmt.bind_blob(2, self.session.0.as_bytes())?;
        stmt.bind_user_id(3, ctx.user_id)?;
        stmt.exec()?;
        Ok(())
    }
}
