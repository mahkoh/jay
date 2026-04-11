use {
    crate::{
        sm::{
            SessionGetStatus, SessionId, SessionManager, ToplevelSession, ToplevelSessionId,
            ToplevelSessionName,
            sm_jobs::{
                SmDbStateHolder, SmJob, SmPending,
                sm_common::{CreateDbStateError, SessionOwnerError},
            },
            sm_wire::sm_wire_toplevel::{
                DeserializeToplevelError, SmToplevelIn, SmToplevelOut, deserialize_toplevel,
                serialize_toplevel,
            },
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork, sqlite_api::SqliteStep},
        utils::stack::Stack,
    },
    std::{
        rc::{Rc, Weak},
        sync::{Arc, LazyLock},
    },
    thiserror::Error,
};

pub struct ToplevelAcquireJob {
    pub work: ToplevelAcquireWork,
    pending: Weak<SmPending<Self>>,
}

pub struct ToplevelAcquireWork {
    pub restore: bool,
    pub session_id: SessionId,
    pub name: ToplevelSessionName,
    pub name_text: String,
    result: Option<Result<ToplevelAcquireOutcome, ToplevelAcquireError>>,
    db_state: Arc<SmDbStateHolder>,
}

struct ToplevelAcquireOutcome {
    id: ToplevelSessionId,
    restored: Option<SmToplevelOut>,
}

#[derive(Debug, Error)]
enum ToplevelAcquireError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error(transparent)]
    Owner(#[from] SessionOwnerError),
    #[error(transparent)]
    Deserialize(#[from] DeserializeToplevelError),
}

impl SmJob for ToplevelAcquireJob {
    type Cb = ToplevelSession;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            work: ToplevelAcquireWork {
                restore: false,
                session_id: SessionId(-1),
                name: ToplevelSessionName(Default::default()),
                name_text: Default::default(),
                result: None,
                db_state,
            },
            pending,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.toplevel_acquire_jobs
    }
}

impl SqliteJob for ToplevelAcquireJob {
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
        match res {
            Ok(o) => {
                cb.id.set(Some(o.id));
                let restored = o.restored.is_some();
                if let Some(v) = o.restored {
                    cb.state.output.set(v.output);
                    cb.state.workspace.set(v.workspace.map(Rc::new));
                    cb.state.floating_pos.set(v.floating_pos);
                    cb.state.fullscreen.set(v.fullscreen);
                }
                if let Some(owner) = cb.owner.get() {
                    owner.loaded(SessionGetStatus::from_restored(restored));
                }
                cb.schedule_job(true);
            }
            Err(e) => {
                if let ToplevelAcquireError::Owner(SessionOwnerError::NotOwned) = e {
                    cb.session.replaced_external();
                    return;
                }
                cb.fatal(e);
            }
        }
    }
}

impl SqliteWork for ToplevelAcquireWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        self.result = Some(self.try_run(ctx));
    }
}

impl ToplevelAcquireWork {
    fn try_run(
        &mut self,
        ctx: &mut SqliteCtx,
    ) -> Result<ToplevelAcquireOutcome, ToplevelAcquireError> {
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_write()?;
        assert_session_owner!(s, self.session_id, ctx.user_id)?;
        let mut stmt = s.s.toplevel_acquire.activate();
        stmt.bind_user_id(1, ctx.user_id)?;
        stmt.bind_blob(2, &self.name.0)?;
        let outcome = match stmt.step()? {
            SqliteStep::Row => {
                // https://gitlab.freedesktop.org/wayland/wayland-protocols/-/work_items/317
                // if !self.restore {
                //     return Err(ToplevelAcquireError::Exists);
                // }
                let id = stmt.get_i64(0)?;
                let data = stmt.get_blob(1)?;
                let tl = deserialize_toplevel(data)?;
                stmt.exec()?;
                ToplevelAcquireOutcome {
                    id: ToplevelSessionId(id),
                    restored: self.restore.then_some(tl),
                }
            }
            SqliteStep::Done => {
                drop(stmt);
                static DEFAULT_DATA: LazyLock<Vec<u8>> = LazyLock::new(|| {
                    let mut data = Vec::new();
                    serialize_toplevel(&mut data, &SmToplevelIn::default());
                    data
                });
                let mut stmt = s.s.toplevel_insert.activate();
                stmt.bind_i64(1, self.session_id.0)?;
                stmt.bind_user_id(2, ctx.user_id)?;
                stmt.bind_blob(3, &self.name.0)?;
                stmt.bind_blob(4, self.name_text.as_bytes())?;
                stmt.bind_blob(5, &DEFAULT_DATA)?;
                stmt.step()?;
                let id = stmt.get_i64(0)?;
                stmt.exec()?;
                s.toplevels_created += 1;
                ToplevelAcquireOutcome {
                    id: ToplevelSessionId(id),
                    restored: None,
                }
            }
        };
        tx.commit()?;
        s.run_gc(&mut ctx.tx);
        Ok(outcome)
    }
}
