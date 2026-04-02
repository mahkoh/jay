use {
    crate::{
        sqlite::{
            SqliteError, SqliteUserId,
            sqlite_sys::{
                self, SQLITE_BLOB, SQLITE_DONE, SQLITE_INTEGER, SQLITE_PREPARE_PERSISTENT,
                SQLITE_ROW, SQLITE_STATIC, SqliteResult, sqlite3, sqlite3_stmt,
            },
        },
        utils::errorfmt::ErrorFmt,
    },
    opera::PhantomNotSync,
    sqlite_sys::Sqlite,
    std::{ptr, rc::Rc, slice},
    uapi::{IntoUstr, c::c_int},
};

pub struct SqliteDb {
    pub api: &'static Sqlite,
    pub s: *mut sqlite3,
    pub _no_sync: PhantomNotSync,
}

enum DbRef<'a> {
    Rc(#[expect(dead_code)] Rc<SqliteDb>),
    Ref(#[expect(dead_code)] &'a SqliteDb),
}

pub struct SqliteStmt<'a> {
    api: &'static Sqlite,
    _db: DbRef<'a>,
    stmt: *mut sqlite3_stmt,
}

pub struct SqliteActiveStmt<'bind, 'stmt> {
    api: &'static Sqlite,
    s: &'bind SqliteStmt<'stmt>,
    state: StatementState,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum StatementState {
    Init,
    Row,
    Done,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SqliteStep {
    Row,
    Done,
}

unsafe impl Send for SqliteDb {}

impl Drop for SqliteDb {
    fn drop(&mut self) {
        let res = unsafe { (self.api.sqlite3_close)(self.s).result() };
        if let Err(e) = res {
            log::error!("Could not close database: {}", ErrorFmt(e));
        }
    }
}

impl Drop for SqliteStmt<'_> {
    fn drop(&mut self) {
        let res = unsafe { (self.api.sqlite3_finalize)(self.stmt).result() };
        if let Err(e) = res {
            log::error!("Could not finalize statement: {}", ErrorFmt(e));
        }
    }
}

impl<'bind, 'stmt> Drop for SqliteActiveStmt<'bind, 'stmt> {
    fn drop(&mut self) {
        if let Err(e) = self.exec_() {
            log::error!("Could not step statement: {}", ErrorFmt(e));
        }
        let res = unsafe { (self.api.sqlite3_reset)(self.s.stmt).result() };
        if let Err(e) = res {
            log::error!("Could not reset statement: {}", ErrorFmt(e));
        }
        let res = unsafe { (self.api.sqlite3_clear_bindings)(self.s.stmt).result() };
        if let Err(e) = res {
            log::error!("Could not clear bindings: {}", ErrorFmt(e));
        }
    }
}

impl SqliteDb {
    pub fn exec(&self, sql: &str) -> Result<(), SqliteError> {
        self.exec_(sql).map_err(SqliteError::Exec)
    }

    pub(super) fn exec_(&self, sql: &str) -> Result<(), SqliteResult> {
        let sql = sql.into_ustr();
        unsafe {
            (self.api.sqlite3_exec)(self.s, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut())
                .result()
        }
    }

    pub fn prepare_tmp(&self, sql: &str) -> Result<SqliteStmt<'_>, SqliteError> {
        let stmt = self.prepare_raw(sql)?;
        Ok(SqliteStmt {
            api: self.api,
            _db: DbRef::Ref(self),
            stmt,
        })
    }

    pub fn prepare(self: &Rc<Self>, sql: &str) -> Result<SqliteStmt<'static>, SqliteError> {
        let stmt = self.prepare_raw(sql)?;
        Ok(SqliteStmt {
            api: self.api,
            _db: DbRef::Rc(self.clone()),
            stmt,
        })
    }

    fn prepare_raw(&self, sql: &str) -> Result<*mut sqlite3_stmt, SqliteError> {
        let n_bytes = sql
            .len()
            .try_into()
            .map_err(|_| SqliteError::StatementTooLong)?;
        let mut stmt = ptr::null_mut();
        unsafe {
            (self.api.sqlite3_prepare_v3)(
                self.s,
                sql.as_ptr().cast(),
                n_bytes,
                SQLITE_PREPARE_PERSISTENT,
                &mut stmt,
                ptr::null_mut(),
            )
            .result()
            .map_err(SqliteError::PrepareStatement)?;
        }
        if stmt.is_null() {
            return Err(SqliteError::NullStatement);
        }
        Ok(stmt)
    }
}

impl<'stmt> SqliteStmt<'stmt> {
    pub fn activate(&mut self) -> SqliteActiveStmt<'_, 'stmt> {
        SqliteActiveStmt {
            s: self,
            api: self.api,
            state: StatementState::Init,
        }
    }

    pub fn exec(&mut self) -> Result<(), SqliteError> {
        self.activate().exec()
    }
}

impl<'bind, 'stmt> SqliteActiveStmt<'bind, 'stmt> {
    pub fn bind_user_id(&self, idx: c_int, v: SqliteUserId) -> Result<(), SqliteError> {
        self.bind_i64(idx, v.0)
    }

    pub fn bind_i64(&self, idx: c_int, v: i64) -> Result<(), SqliteError> {
        if self.state != StatementState::Init {
            return Err(SqliteError::NotInit);
        }
        unsafe {
            (self.api.sqlite3_bind_int64)(self.s.stmt, idx, v)
                .result()
                .map_err(SqliteError::BindI64)?;
        }
        Ok(())
    }

    pub fn bind_blob<'data>(&self, idx: c_int, data: &'data [u8]) -> Result<(), SqliteError>
    where
        'data: 'bind,
    {
        if self.state != StatementState::Init {
            return Err(SqliteError::NotInit);
        }
        let n = data
            .len()
            .try_into()
            .map_err(|_| SqliteError::BlobTooLong)?;
        unsafe {
            (self.api.sqlite3_bind_blob)(self.s.stmt, idx, data.as_ptr().cast(), n, SQLITE_STATIC)
                .result()
                .map_err(SqliteError::BindBlob)?;
        }
        Ok(())
    }

    pub fn exec(mut self) -> Result<(), SqliteError> {
        self.exec_()
    }

    fn exec_(&mut self) -> Result<(), SqliteError> {
        while self.state != StatementState::Done {
            self.step()?;
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<SqliteStep, SqliteError> {
        if self.state == StatementState::Done {
            return Err(SqliteError::Done);
        }
        let res = unsafe { (self.api.sqlite3_step)(self.s.stmt) };
        if res.0 == SQLITE_ROW {
            self.state = StatementState::Row;
            return Ok(SqliteStep::Row);
        }
        self.state = StatementState::Done;
        if res.0 == SQLITE_DONE {
            return Ok(SqliteStep::Done);
        }
        Err(SqliteError::Step(res))
    }

    pub fn get_i64(&self, idx: c_int) -> Result<i64, SqliteError> {
        if self.state != StatementState::Row {
            return Err(SqliteError::NotRow);
        }
        let ty = unsafe { (self.api.sqlite3_column_type)(self.s.stmt, idx) };
        if ty != SQLITE_INTEGER {
            return Err(SqliteError::NotI64);
        }
        let res = unsafe { (self.api.sqlite3_column_int64)(self.s.stmt, idx) };
        Ok(res)
    }

    #[expect(dead_code)]
    pub fn get_blob(&self, idx: c_int) -> Result<&[u8], SqliteError> {
        if self.state != StatementState::Row {
            return Err(SqliteError::NotRow);
        }
        let ty = unsafe { (self.api.sqlite3_column_type)(self.s.stmt, idx) };
        if ty != SQLITE_BLOB {
            return Err(SqliteError::NotBlob);
        }
        let len = unsafe { (self.api.sqlite3_column_bytes)(self.s.stmt, idx) };
        let Ok(len) = len.try_into() else {
            return Err(SqliteError::NegativeBlobSize);
        };
        let bytes = unsafe { (self.api.sqlite3_column_blob)(self.s.stmt, idx) };
        if bytes.is_null() {
            return Ok(&[]);
        }
        let slice = unsafe { slice::from_raw_parts(bytes.cast(), len) };
        Ok(slice)
    }
}
