use {
    crate::{
        sm::{
            Session, SessionGetStatus, SessionId, SessionManager, SessionName, session_name,
            sm_jobs::{
                SmJob, SmPending,
                sm_common::{CreateDbStateError, SmDbStateHolder},
            },
            sm_wire::sm_wire_session::{
                DeserializeSessionError, SmSessionIn, SmSessionOut, patch_session,
                serialize_session,
            },
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork, sqlite_api::SqliteStep},
        utils::stack::Stack,
    },
    std::{
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
};

pub struct SessionAcquireJob {
    pub work: SessionAcquireWork,
    pending: Weak<SmPending<Self>>,
}

pub struct SessionAcquireWork {
    pub session: SessionName,
    pub data: SmSessionIn,
    pub restore: bool,
    db_state: Arc<SmDbStateHolder>,
    stash: Vec<u8>,
    result: Option<Result<SessionAcquireOutcome, SessionUpsertError>>,
}

struct SessionAcquireOutcome {
    id: SessionId,
    restored: Option<SmSessionOut>,
}

#[derive(Debug, Error)]
enum SessionUpsertError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error(transparent)]
    Deserialize(#[from] DeserializeSessionError),
}

impl SmJob for SessionAcquireJob {
    type Cb = Session;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            work: SessionAcquireWork {
                session: session_name(),
                data: Default::default(),
                restore: Default::default(),
                db_state,
                stash: Default::default(),
                result: Default::default(),
            },
            pending,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.session_acquire_jobs
    }
}

impl SqliteJob for SessionAcquireJob {
    fn work(&mut self) -> &mut dyn SqliteWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        let result = self.work.result.take().unwrap();
        let Some(pending) = self.pending.upgrade() else {
            return;
        };
        pending.job.set(Some(self));
        let Some(cb) = pending.cb.take() else {
            return;
        };
        cb.job.take();
        match result {
            Ok(v) => {
                cb.id.set(Some(v.id));
                if let Some(owner) = cb.owner.get() {
                    owner.loaded(SessionGetStatus::from_restored(v.restored.is_some()));
                }
                cb.schedule_jobs();
            }
            Err(e) => cb.fatal(e),
        }
    }
}

impl SqliteWork for SessionAcquireWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        self.result = Some(self.try_run(ctx));
    }
}

impl SessionAcquireWork {
    fn try_run(
        &mut self,
        ctx: &mut SqliteCtx,
    ) -> Result<SessionAcquireOutcome, SessionUpsertError> {
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_write()?;
        let restored = if !self.restore {
            let mut stmt = s.s.session_del_unchecked.activate();
            stmt.bind_blob(1, self.session.0.as_bytes())?;
            if stmt.step()? == SqliteStep::Done {
                s.sessions_created += 1;
            }
            stmt.exec()?;
            serialize_session(&mut self.stash, &self.data);
            None
        } else {
            let mut stmt = s.s.session_load.activate();
            stmt.bind_blob(1, self.session.0.as_bytes())?;
            let res = match stmt.step()? {
                SqliteStep::Row => {
                    let data = stmt.get_blob(0)?;
                    Some(patch_session(&mut self.stash, data, &self.data)?)
                }
                SqliteStep::Done => {
                    s.sessions_created += 1;
                    serialize_session(&mut self.stash, &self.data);
                    None
                }
            };
            stmt.exec()?;
            res
        };
        let id = {
            let mut stmt = s.s.session_upsert.activate();
            stmt.bind_blob(1, self.session.0.as_bytes())?;
            stmt.bind_user_id(2, ctx.user_id)?;
            stmt.bind_blob(3, &self.stash)?;
            stmt.step()?;
            let id = stmt.get_i64(0)?;
            stmt.exec()?;
            id
        };
        tx.commit()?;
        s.run_gc(&mut ctx.tx);
        Ok(SessionAcquireOutcome {
            id: SessionId(id),
            restored,
        })
    }
}
