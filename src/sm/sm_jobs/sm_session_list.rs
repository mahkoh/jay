use {
    crate::{
        sm::{
            SessionList, SessionListSession, SessionListToplevel, SessionManager, SessionName,
            ToplevelSessionName,
            sm_jobs::{SmDbStateHolder, SmJob, SmPending, sm_common::CreateDbStateError},
            sm_wire::{
                sm_wire_session::{DeserializeSessionError, deserialize_session},
                sm_wire_toplevel::{DeserializeToplevelError, deserialize_toplevel},
            },
        },
        sqlite::{SqliteCtx, SqliteError, SqliteJob, SqliteWork, sqlite_api::SqliteStep},
        utils::{errorfmt::ErrorFmt, opaque::Opaque, stack::Stack, tri::Try},
    },
    ahash::AHashMap,
    std::{
        cell::Cell,
        error::Error,
        rc::{Rc, Weak},
        sync::Arc,
        time::{Duration, SystemTime},
    },
    thiserror::Error,
};

pub struct SessionListJob {
    pub work: SessionListWork,
    pending: Weak<SmPending<Self>>,
}

pub struct SessionListWork {
    result: Option<Result<SessionList, SessionListError>>,
    db_state: Arc<SmDbStateHolder>,
}

pub struct SessionListRequest {
    pub cb: Cell<Option<Box<dyn FnOnce(Result<SessionList, Box<dyn Error>>)>>>,
}

#[derive(Debug, Error)]
enum SessionListError {
    #[error(transparent)]
    CreateDbState(#[from] CreateDbStateError),
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error("The session column has a wrong length")]
    WrongSessionLength,
    #[error("The name column has a wrong length")]
    WrongToplevelLength,
    #[error("The toplevel name is not UTF-8")]
    ToplevelNameUtf8,
    #[error("The access time overflowed")]
    AtimeOverflow,
    #[error("Toplevel references an unknown session")]
    UnknownSession,
    #[error(transparent)]
    DeserializeSession(#[from] DeserializeSessionError),
    #[error(transparent)]
    DeserializeToplevel(#[from] DeserializeToplevelError),
}

impl SmJob for SessionListJob {
    type Cb = SessionListRequest;

    fn new(db_state: Arc<SmDbStateHolder>, pending: Weak<SmPending<Self>>) -> Self {
        Self {
            work: SessionListWork {
                result: Default::default(),
                db_state,
            },
            pending,
        }
    }

    fn stack(sm: &SessionManager) -> &Stack<Rc<SmPending<Self>>> {
        &sm.session_list_jobs
    }
}

impl SqliteJob for SessionListJob {
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
        let Some(cb) = cb.cb.take() else {
            return;
        };
        cb(result.map_err(Box::new).map_err(Into::into));
    }
}

impl SqliteWork for SessionListWork {
    fn run(&mut self, ctx: &mut SqliteCtx) {
        self.result = Some(self.try_run(ctx));
    }
}

impl SessionListWork {
    fn try_run(&mut self, ctx: &mut SqliteCtx) -> Result<SessionList, SessionListError> {
        db_state!(s, self, ctx);
        let tx = ctx.tx.begin_read()?;
        let parse_ts = |ts: i64| {
            let ts = if ts < 0 {
                SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs((-ts) as u64))
            } else {
                SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(ts as u64))
            };
            ts.ok_or(SessionListError::AtimeOverflow)
        };
        let mut sessions = AHashMap::new();
        let mut stmt = s.s.session_list.activate();
        while stmt.step()? == SqliteStep::Row {
            let res = SessionListError::tri(|| {
                let session_id = stmt.get_i64(0)?;
                let session = stmt.get_blob(1)?;
                let data = stmt.get_blob(2)?;
                let ctime = stmt.get_i64(3)?;
                let atime = stmt.get_i64(4)?;
                let Ok(session) = session.try_into() else {
                    return Err(SessionListError::WrongSessionLength);
                };
                let session = SessionName(Opaque::from_bytes(session));
                let data = deserialize_session(data)?;
                let ctime = parse_ts(ctime)?;
                let atime = parse_ts(atime)?;
                sessions.insert(
                    session_id,
                    SessionListSession {
                        name: session,
                        data,
                        ctime,
                        atime,
                        toplevels: Default::default(),
                    },
                );
                Ok(())
            });
            if let Err(e) = res {
                log::error!("Could not deserialize session: {}", ErrorFmt(e));
            }
        }
        let mut stmt = s.s.toplevel_list.activate();
        while stmt.step()? == SqliteStep::Row {
            let res = SessionListError::tri(|| {
                let session_id = stmt.get_i64(0)?;
                let name = stmt.get_blob(1)?;
                let name_text = stmt.get_blob(2)?;
                let data = stmt.get_blob(3)?;
                let ctime = stmt.get_i64(4)?;
                let atime = stmt.get_i64(5)?;
                let Ok(name) = name.try_into() else {
                    return Err(SessionListError::WrongToplevelLength);
                };
                let Ok(name_text) = str::from_utf8(name_text) else {
                    return Err(SessionListError::ToplevelNameUtf8);
                };
                let name = ToplevelSessionName(name);
                let data = deserialize_toplevel(data)?;
                let ctime = parse_ts(ctime)?;
                let atime = parse_ts(atime)?;
                let data = SessionListToplevel {
                    name,
                    name_text: name_text.to_string(),
                    data,
                    ctime,
                    atime,
                };
                let Some(session) = sessions.get_mut(&session_id) else {
                    return Err(SessionListError::UnknownSession);
                };
                session.toplevels.push(data);
                Ok(())
            });
            if let Err(e) = res {
                log::error!("Could not deserialize toplevel: {}", ErrorFmt(e));
            }
        }
        drop(tx);
        let mut sessions: Vec<_> = sessions.into_values().collect();
        sessions.sort_by_key(|s| s.name);
        for s in &mut sessions {
            s.toplevels.sort_by(|a, b| a.name_text.cmp(&b.name_text));
        }
        Ok(SessionList { sessions })
    }
}
