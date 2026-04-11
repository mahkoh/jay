use {
    crate::{
        sm::SessionId,
        sqlite::{
            Sqlite, SqliteError, SqliteTransactionManager, SqliteUserId,
            sqlite_api::{SqliteDb, SqliteStep, SqliteStmt},
        },
        utils::{errorfmt::ErrorFmt, thread_local_data::ThreadLocalData},
    },
    arrayvec::ArrayVec,
    std::{cell::RefCell, rc::Rc, time::Instant},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum SessionOwnerError {
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error("The session is not owned by the current Jay instance")]
    NotOwned,
}

pub fn assert_session_owner(
    stmt: &mut SqliteStmt<'static>,
    id: SessionId,
    user_id: SqliteUserId,
) -> Result<(), SessionOwnerError> {
    let mut stmt = stmt.activate();
    stmt.bind_i64(1, id.0)?;
    stmt.bind_user_id(2, user_id)?;
    stmt.step()?;
    if stmt.get_i64(0)? == 0 {
        return Err(SessionOwnerError::NotOwned);
    }
    stmt.exec()?;
    Ok(())
}

macro_rules! assert_session_owner {
    ($s:expr, $id:expr, $user_id:expr) => {
        crate::sm::sm_jobs::sm_common::assert_session_owner(&mut $s.s.session_owned, $id, $user_id)
    };
}

pub struct SmDbStateHolder {
    state: ThreadLocalData<RefCell<SmDbState>>,
}

pub struct SmDbState {
    pub sessions_created: u64,
    pub toplevels_created: u64,
    pub s: SmDbStatements,
}

macro_rules! statements {
    ($($name:ident,)*) => {
        pub struct SmDbStatements {
            $(pub $name: SqliteStmt<'static>,)*
        }

        impl SmDbStatements {
            fn load(db: &Rc<SqliteDb>) -> Result<Self, SqliteError> {
                Ok(Self {
                    $(
                        $name: db.prepare(include_str!(concat!("../sql/", stringify!($name), ".sql")))?,
                    )*
                })
            }
        }
    };
}

statements! {
    session_del,
    session_del_in_age_class,
    session_del_unchecked,
    session_disown,
    session_list,
    session_list_by_age_class,
    session_load,
    session_owned,
    session_upsert,
    toplevel_acquire,
    toplevel_del,
    toplevel_del_in_age_class,
    toplevel_disown_all,
    toplevel_disown_one,
    toplevel_insert,
    toplevel_list,
    toplevel_list_by_age_class,
    toplevel_rename,
    toplevel_store,
}

impl SmDbStateHolder {
    pub fn new(sqlite: &Sqlite) -> Self {
        Self {
            state: ThreadLocalData::new(sqlite.thread_id),
        }
    }
}

impl SmDbState {
    fn new(db: &Rc<SqliteDb>, tx: &mut SqliteTransactionManager) -> Result<Self, SqliteError> {
        db.exec(include_str!("../sql/init.sql"))?;
        let mut slf = Self {
            sessions_created: !0,
            toplevels_created: !0,
            s: SmDbStatements::load(db)?,
        };
        slf.run_gc(tx);
        Ok(slf)
    }
}

#[derive(Debug, Error)]
#[error("Could not create DB state")]
pub struct CreateDbStateError(#[source] SqliteError);

impl SmDbStateHolder {
    pub fn get(
        &self,
        db: &Rc<SqliteDb>,
        tx: &mut SqliteTransactionManager,
    ) -> Result<Rc<RefCell<SmDbState>>, CreateDbStateError> {
        self.state.get_or_try_create(|| {
            SmDbState::new(db, tx)
                .map(RefCell::new)
                .map(Rc::new)
                .map_err(CreateDbStateError)
        })
    }
}

macro_rules! db_state {
    ($s:ident, $slf:expr, $ctx:expr) => {
        let state = $slf.db_state.get(&$ctx.db, &mut $ctx.tx)?;
        let $s = &mut *state.borrow_mut();
    };
}

impl SmDbState {
    pub fn run_gc(&mut self, tx: &mut SqliteTransactionManager) {
        if let Err(e) = self.run_gc_(tx) {
            log::error!("Could not run GC: {}", ErrorFmt(e));
        }
    }

    fn run_gc_(&mut self, tx: &mut SqliteTransactionManager) -> Result<(), SqliteError> {
        // We use the following garbage collection algorithm for sessions/toplevels in the
        // database.
        //
        // First of all, objects that are currently being used by any compositior are
        // never garbage collected. It is up to the compositor to limit the number of such
        // objects in another way.
        //
        // All other rows are split into age classes. The age classes are determined as
        // follows:
        //
        //   floor(log2(max(16, atime - ctime))) - 4
        //
        // Where atime is the last time the row was accessed and ctime is the time when
        // the row was created; both in seconds.
        //
        // This means that the age class is a function of the duration between first use
        // and last use. An object created a year ago and accessed once within 3 seconds
        // falls into the same age class as an object created 3 seconds ago.
        //
        // The function evaluates to 0 for everything under 32 seconds. After that, the
        // upper bound of each age class is twice its lower bound. That is, the age
        // classes are
        //
        //           0s -        32s
        //          32s -     1m  4s
        //       1m  4s -     2m  8s
        //       2m  8s -     4m 16s
        //       4m 16s -     8m 32s
        //       8m 32s -    17m  4s
        //      17m  4s -    34m  8s
        //      34m  8s - 1h  8m 16s
        //   1h  8m 16s - 2h 16m 32s
        //   2h 16m 32s - 4h 33m  4s
        //
        // and so on.
        //
        // Each age class is garbage collected independently. This means that a flood of
        // short-lived sessions will never cause long-lived sessions to be deleted.
        //
        // The maximum number of sessions within each age class is calculated as follows:
        //
        //    256 * pow(1.27, min(age_class, 26))
        //
        // where age_class is the index of the age class, starting at 0. For example,
        //
        //                0s -                32s:    256
        //               32s -             1m  4s:    325
        //            1m  4s -             2m  8s:    412
        //            2m  8s -             4m 16s:    524
        //            4m 16s -             8m 32s:    665
        //            8m 32s -            17m  4s:    845
        //           17m  4s -            34m  8s:   1074
        //           34m  8s -         1h  8m 16s:   1364
        //        1h  8m 16s -         2h 16m 32s:   1732
        //        2h 16m 32s -         4h 33m  4s:   2200
        //                  ...
        // 34y 9d 1h 37m  4s - 68y 18d 3h 14m  8s: 127978
        //
        // If the number of rows in an age class exceeds this, we delete the row with the
        // oldest atime until the number of rows no longer exceeds the maximum.
        //
        // The maximum number of toplevels is similarly calculated via
        //
        //    1024 * pow(1.3, min(age_class, 26))
        //
        // which yields
        //
        //                0s -                32s:   1024
        //               32s -             1m  4s:   1331
        //            1m  4s -             2m  8s:   1730
        //            2m  8s -             4m 16s:   2249
        //            4m 16s -             8m 32s:   2924
        //            8m 32s -            17m  4s:   3802
        //           17m  4s -            34m  8s:   4942
        //           34m  8s -         1h  8m 16s:   6425
        //        1h  8m 16s -         2h 16m 32s:   8353
        //        2h 16m 32s -         4h 33m  4s:  10859
        //                  ...
        // 34y 9d 1h 37m  4s - 68y 18d 3h 14m  8s: 939349
        //
        // If we use 10 years as a reasonable upper bound for the lifetime of the database
        // and sum the sizes of the age classes, we arrive at a maximum number of sessions
        // of 372_263 and a maximum number of toplevels of 2_405_163. These values can be
        // used to estimate the maximum size of the database.

        const SESSION_MUL: f64 = 256.0;
        const SESSION_BASE: f64 = 1.27;
        const TOPLEVEL_MUL: f64 = 1024.0;
        const TOPLEVEL_BASE: f64 = 1.3;
        const MAX_AGE_CLASS: i64 = 26;
        const SESSION_GC_INTERVAL: u64 = 256;
        const TOPLEVEL_GC_INTERVAL: u64 = 1024;

        let mut types = ArrayVec::<_, 2>::new();
        if self.sessions_created >= SESSION_GC_INTERVAL {
            self.sessions_created = 0;
            types.push((
                &mut self.s.session_list_by_age_class,
                &mut self.s.session_del_in_age_class,
                SESSION_MUL,
                SESSION_BASE,
            ));
        }
        if self.toplevels_created >= TOPLEVEL_GC_INTERVAL {
            self.toplevels_created = 0;
            types.push((
                &mut self.s.toplevel_list_by_age_class,
                &mut self.s.toplevel_del_in_age_class,
                TOPLEVEL_MUL,
                TOPLEVEL_BASE,
            ));
        }
        if types.is_empty() {
            return Ok(());
        }
        let start = Instant::now();
        let tx = tx.begin_write()?;
        for (list, del, mul, base) in types {
            let mut stmt = list.activate();
            while stmt.step()? == SqliteStep::Row {
                let age_class = stmt.get_i64(0)?;
                let count = stmt.get_i64(1)?;
                let size_class = age_class.min(MAX_AGE_CLASS);
                let size = (mul * base.powf(size_class as f64)) as i64;
                if count > size {
                    let stmt = del.activate();
                    stmt.bind_i64(1, age_class)?;
                    stmt.bind_i64(2, count - size)?;
                    stmt.exec()?;
                }
            }
        }
        tx.commit()?;
        log::info!("GC took {:?}", start.elapsed());
        Ok(())
    }
}
