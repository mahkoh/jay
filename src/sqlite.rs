use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::IoUring,
        sqlite::{
            sqlite_api::{SqliteDb, SqliteStmt},
            sqlite_sys::{
                SQLITE, SQLITE_CONFIG_LOG, SQLITE_OPEN_CREATE, SQLITE_OPEN_EXRESCODE,
                SQLITE_OPEN_NOMUTEX, SQLITE_OPEN_READWRITE, SqliteResult,
            },
        },
        state::State,
        utils::{
            buf::Buf,
            data_dir::data_dir,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            opaque::{OPAQUE_LEN, opaque},
            oserror::{OsError, OsErrorExt, OsErrorExt2},
            ptr_ext::MutPtrExt,
            queue::AsyncQueue,
            syncqueue::SyncQueue,
            thread_id::ThreadId,
        },
    },
    arrayvec::ArrayString,
    parking_lot::{Condvar, Mutex},
    std::{
        cell::Cell,
        collections::VecDeque,
        ffi::{CStr, c_void},
        io, mem, ptr,
        rc::Rc,
        sync::{
            Arc, Once,
            atomic::{
                AtomicU64,
                Ordering::{Acquire, Release},
            },
        },
        thread::{self, JoinHandle},
        time::Duration,
    },
    thiserror::Error,
    uapi::{
        AsUstr, IntoUstr, OwnedFd,
        c::{
            self, AT_EMPTY_PATH, AT_FDCWD, LOCK_EX, LOCK_NB, O_CLOEXEC, O_DIRECTORY, O_PATH,
            O_RDONLY, O_RDWR, O_TMPFILE, c_char, c_int,
        },
    },
};

pub mod sqlite_api;
pub mod sqlite_sys;

#[derive(Debug, Error)]
pub enum SqliteError {
    #[error("Could not load sqlite")]
    Load,
    #[error("Sqlite thread has exited")]
    Dead,
    #[error("Could not open the database")]
    Open(#[source] SqliteResult),
    #[error("Could not create a lock file")]
    CreateLockFile(#[source] LockFileError),
    #[error("Could not list lock files")]
    ListLockFiles(#[source] LockFileError),
    #[error("Could not initialize the database")]
    InitDb(#[source] SqliteResult),
    #[error("Could not create an eventfd")]
    CreateEventfd(#[source] OsError),
    #[error("Could not dup an eventfd")]
    DupEventfd(#[source] OsError),
    #[error("The statement is too long")]
    StatementTooLong,
    #[error("The blob is too long")]
    BlobTooLong,
    #[error("Could not prepare statement")]
    PrepareStatement(#[source] SqliteResult),
    #[error("The statement is empty")]
    NullStatement,
    #[error("Could not bind blob")]
    BindBlob(#[source] SqliteResult),
    #[error("Could not bind i64")]
    BindI64(#[source] SqliteResult),
    #[error("Active statement is not in initial state")]
    NotInit,
    #[error("Active statement is not in row state")]
    NotRow,
    #[error("Active statement is done")]
    Done,
    #[error("Could not step")]
    Step(#[source] SqliteResult),
    #[error("Data type is not a blob")]
    NotBlob,
    #[error("Data type is not an i64")]
    NotI64,
    #[error("Blob size is negative")]
    NegativeBlobSize,
    #[error("Could not exec")]
    Exec(#[source] SqliteResult),
    #[error("Too many outstanding sqlite jobs")]
    UsageExceeded,
}

pub struct Sqlite {
    join_handle: Cell<Option<JoinHandle<()>>>,
    tasks: Cell<Option<[SpawnedFuture<()>; 2]>>,
    thread: Arc<SqliteThread>,
    ring: Rc<IoUring>,
    eventfd: Rc<OwnedFd>,
    next_id: NumCell<u64>,
    pending: SyncQueue<Pending>,
    requests: AsyncQueue<Request>,
    _lock_file: LockFile,
    pub thread_id: ThreadId,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SqliteUserId(i64);

struct Pending {
    id: u64,
    job: *mut dyn SqliteJob,
    _usage: Option<SqliteUsage>,
}

enum SqliteThreadStatus {
    Pending,
    Init(ThreadId),
    Dead,
}

struct SqliteThread {
    data: Mutex<SqliteThreadData>,
    cv: Condvar,
    eventfd: OwnedFd,
    last_id: AtomicU64,
}

struct SqliteThreadData {
    requests: VecDeque<Request>,
    waiting: bool,
    status: SqliteThreadStatus,
}

pub struct SqliteTransactionManager {
    begin: SqliteStmt<'static>,
    begin_immediate: SqliteStmt<'static>,
    commit: SqliteStmt<'static>,
    rollback: SqliteStmt<'static>,
}

pub struct SqliteReadTransaction<'a> {
    mgr: &'a mut SqliteTransactionManager,
}

pub struct SqliteWriteTransaction<'a> {
    tx: SqliteReadTransaction<'a>,
}

pub struct SqliteCtx {
    pub db: Rc<SqliteDb>,
    pub tx: SqliteTransactionManager,
    pub user_id: SqliteUserId,
}

enum Request {
    Exit,
    Sync {
        done: Arc<Mutex<bool>>,
        cv: Arc<Condvar>,
    },
    Work {
        id: u64,
        work: &'static mut dyn SqliteWork,
    },
}

pub trait SqliteJob {
    fn work(&mut self) -> &mut dyn SqliteWork;
    fn completed(self: Box<Self>);
}

pub trait SqliteWork: Send {
    fn run(&mut self, ctx: &mut SqliteCtx);
}

struct Exit;

pub struct SqliteAccounting {
    cur: NumCell<usize>,
    max: usize,
}

pub struct SqliteUsage {
    accounting: Rc<SqliteAccounting>,
}

impl SqliteAccounting {
    pub fn new(max: usize) -> SqliteAccounting {
        Self {
            cur: Default::default(),
            max,
        }
    }

    pub fn reserve(self: &Rc<Self>) -> Result<SqliteUsage, SqliteError> {
        let cur = self.cur.get();
        if cur >= self.max {
            return Err(SqliteError::UsageExceeded);
        }
        self.cur.set(cur + 1);
        Ok(SqliteUsage {
            accounting: self.clone(),
        })
    }
}

impl Drop for SqliteUsage {
    fn drop(&mut self) {
        self.accounting.cur.fetch_sub(1);
    }
}

impl Sqlite {
    pub fn open(
        ring: &Rc<IoUring>,
        eng: &Rc<AsyncEngine>,
        in_memory: bool,
    ) -> Result<Rc<Sqlite>, SqliteError> {
        let Some(api) = &*SQLITE else {
            return Err(SqliteError::Load);
        };
        static INIT_LOG: Once = Once::new();
        INIT_LOG.call_once(|| {
            let res = unsafe {
                (api.sqlite3_config)(
                    SQLITE_CONFIG_LOG,
                    log as unsafe extern "C" fn(*mut c_void, c_int, *const c_char),
                    ptr::null::<c_void>(),
                )
                .result()
            };
            if let Err(e) = res {
                log::warn!("Could not setup sqlite log function: {}", ErrorFmt(e));
            }
        });
        let lock_file = LockFile::new().map_err(SqliteError::CreateLockFile)?;
        let filename = data_dir().join("db/db.sqlite").into_ustr();
        let filename = match in_memory {
            true => c":memory:".as_ptr(),
            false => filename.as_ptr(),
        };
        let mut db = SqliteDb {
            api,
            s: ptr::null_mut(),
            _no_sync: Default::default(),
        };
        unsafe {
            (api.sqlite3_open_v2)(
                filename,
                &mut db.s,
                SQLITE_OPEN_READWRITE
                    | SQLITE_OPEN_CREATE
                    | SQLITE_OPEN_NOMUTEX
                    | SQLITE_OPEN_EXRESCODE,
                ptr::null(),
            )
            .result()
            .map_err(SqliteError::Open)?
        }
        db.exec_(include_str!("sqlite/sql/init.sql"))
            .map_err(SqliteError::InitDb)?;
        db.exec("begin")?;
        let user_id = {
            let mut stmt = db.prepare_tmp(include_str!("sqlite/sql/insert_user.sql"))?;
            let mut stmt = stmt.activate();
            stmt.bind_blob(1, lock_file.name.as_bytes())?;
            stmt.step()?;
            let id = SqliteUserId(stmt.get_i64(0)?);
            stmt.exec()?;
            id
        };
        {
            let locks = lock_file
                .collect_lock_files()
                .map_err(SqliteError::ListLockFiles)?;
            let mut stmt = db.prepare_tmp(include_str!("sqlite/sql/delete_user_to_delete.sql"))?;
            for lock in &locks {
                let stmt = stmt.activate();
                stmt.bind_blob(1, lock.as_bytes())?;
                stmt.exec()?;
            }
            db.exec(include_str!("sqlite/sql/delete_unlocked_users.sql"))?;
        }
        db.exec("commit")?;
        let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC).map_os_err(SqliteError::CreateEventfd)?;
        let eventfd_dup =
            uapi::fcntl_dupfd_cloexec(eventfd.raw(), 0).map_os_err(SqliteError::DupEventfd)?;
        let thread = Arc::new(SqliteThread {
            data: Mutex::new(SqliteThreadData {
                requests: Default::default(),
                waiting: false,
                status: SqliteThreadStatus::Pending,
            }),
            cv: Default::default(),
            eventfd: eventfd_dup,
            last_id: Default::default(),
        });
        let thread1 = thread.clone();
        let join_handle = thread::Builder::new()
            .name("sqlite".to_string())
            .spawn(move || {
                thread1.run(db, user_id);
            })
            .unwrap();
        let thread_id = {
            let mut data = thread.data.lock();
            loop {
                match &data.status {
                    SqliteThreadStatus::Pending => {
                        thread.cv.wait(&mut data);
                    }
                    SqliteThreadStatus::Init(id) => break *id,
                    SqliteThreadStatus::Dead => {
                        join_handle.join().unwrap();
                        return Err(SqliteError::Dead);
                    }
                }
            }
        };
        let slf = Rc::new(Self {
            join_handle: Cell::new(Some(join_handle)),
            requests: Default::default(),
            thread,
            ring: ring.clone(),
            eventfd: Rc::new(eventfd),
            next_id: NumCell::new(1),
            pending: Default::default(),
            _lock_file: lock_file,
            tasks: Default::default(),
            thread_id,
        });
        let tasks = [
            eng.spawn("sqlite-req", slf.clone().handle_requests()),
            eng.spawn("sqlite-res", slf.clone().handle_results()),
        ];
        slf.tasks.set(Some(tasks));
        log::info!("Sqlite initialized");
        Ok(slf)
    }

    pub fn clear(&self) {
        self.tasks.take();
        self.requests.push(Request::Exit);
        self.flush_requests();
        if let Some(jh) = self.join_handle.take() {
            jh.join().unwrap();
        }
        for pending in self.pending.take() {
            let _ = unsafe { Box::from_raw(pending.job) };
        }
    }

    pub fn blocking_roundtrip(&self) {
        let done = Arc::new(Mutex::new(false));
        let cv = Arc::new(Condvar::new());
        self.requests.push(Request::Sync {
            done: done.clone(),
            cv: cv.clone(),
        });
        self.flush_requests();
        let mut done = done.lock();
        while !*done {
            cv.wait(&mut done);
        }
    }

    pub fn add_job(&self, usage: Option<SqliteUsage>, job: Box<dyn SqliteJob>) {
        let job = Box::into_raw(job);
        let id = self.next_id.fetch_add(1);
        self.requests.push(Request::Work {
            id,
            work: unsafe { job.deref_mut().work() },
        });
        self.pending.push(Pending {
            id,
            job,
            _usage: usage,
        });
    }

    async fn handle_requests(self: Rc<Self>) {
        loop {
            self.requests.non_empty().await;
            self.flush_requests();
        }
    }

    fn flush_requests(&self) {
        if self.requests.is_empty() {
            return;
        }
        let waiting = {
            let mut data = self.thread.data.lock();
            self.requests.move_to(&mut data.requests);
            data.waiting
        };
        if waiting {
            self.thread.cv.notify_all();
        }
    }

    async fn handle_results(self: Rc<Self>) {
        let mut buf = Buf::new(size_of::<u64>());
        loop {
            let res = self.ring.read(&self.eventfd, buf.slice(..)).await;
            if let Err(e) = res {
                log::error!("Could not read from eventfd: {}", ErrorFmt(e));
                return;
            }
            let last_id = self.thread.last_id.load(Acquire);
            while let Some(first) = self.pending.pop() {
                if first.id > last_id {
                    self.pending.push_front(first);
                    break;
                }
                let job = unsafe { Box::from_raw(first.job) };
                job.completed();
            }
        }
    }
}

unsafe extern "C" fn log(_ptr: *mut c_void, code: c_int, msg: *const c_char) {
    let code = SqliteResult(code);
    let msg = unsafe { CStr::from_ptr(msg).as_ustr() };
    log::warn!("SQLITE: {code}: {}", msg.display());
}

impl SqliteTransactionManager {
    pub fn begin_read(&mut self) -> Result<SqliteReadTransaction<'_>, SqliteError> {
        self.begin.exec()?;
        Ok(SqliteReadTransaction { mgr: self })
    }

    pub fn begin_write(&mut self) -> Result<SqliteWriteTransaction<'_>, SqliteError> {
        self.begin_immediate.exec()?;
        Ok(SqliteWriteTransaction {
            tx: SqliteReadTransaction { mgr: self },
        })
    }
}

impl SqliteWriteTransaction<'_> {
    pub fn commit(self) -> Result<(), SqliteError> {
        self.tx.mgr.commit.exec()?;
        mem::forget(self);
        Ok(())
    }
}

impl Drop for SqliteReadTransaction<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.mgr.rollback.exec() {
            log::error!("Could not roll back transaction: {}", ErrorFmt(e));
        }
    }
}

impl SqliteThread {
    pub fn run(self: Arc<Self>, db: SqliteDb, user_id: SqliteUserId) {
        let db = Rc::new(db);
        let mgr = match self.create_tx(&db) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not create transaction manager: {}", ErrorFmt(e));
                self.data.lock().status = SqliteThreadStatus::Dead;
                self.cv.notify_all();
                return;
            }
        };
        self.data.lock().status = SqliteThreadStatus::Init(ThreadId::current());
        self.cv.notify_all();
        self.handle_requests(db.clone(), mgr, user_id);
        let res = (|| {
            let mut stmt = db.prepare(include_str!("sqlite/sql/delete_user.sql"))?;
            let stmt = stmt.activate();
            stmt.bind_user_id(1, user_id)?;
            stmt.exec()
        })();
        if let Err(e) = res {
            log::error!("Could not delete user: {}", ErrorFmt(e));
        }
    }

    fn create_tx(&self, db: &Rc<SqliteDb>) -> Result<SqliteTransactionManager, SqliteError> {
        let mgr = SqliteTransactionManager {
            begin: db.prepare("begin")?,
            begin_immediate: db.prepare("begin immediate")?,
            commit: db.prepare("commit")?,
            rollback: db.prepare("rollback")?,
        };
        Ok(mgr)
    }

    fn handle_requests(
        &self,
        db: Rc<SqliteDb>,
        tx: SqliteTransactionManager,
        user_id: SqliteUserId,
    ) -> Exit {
        let mut ctx = SqliteCtx {
            db: db.clone(),
            tx,
            user_id,
        };
        let mut requests = VecDeque::new();
        loop {
            {
                let mut lock = self.data.lock();
                if lock.requests.is_empty() {
                    lock.waiting = true;
                    self.cv.wait(&mut lock);
                    lock.waiting = false;
                    continue;
                }
                mem::swap(&mut requests, &mut lock.requests);
            }
            while let Some(request) = requests.pop_front() {
                match request {
                    Request::Exit => return Exit,
                    Request::Sync { done, cv } => {
                        *done.lock() = true;
                        cv.notify_all();
                    }
                    Request::Work { id, work } => {
                        work.run(&mut ctx);
                        self.last_id.store(id, Release);
                    }
                }
            }
            if let Err(e) = uapi::eventfd_write(self.eventfd.raw(), 1).to_os_error() {
                log::error!("Could not write eventfd: {}", ErrorFmt(e));
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum LockFileError {
    #[error("Could not create locks dir")]
    CreateDir(#[source] io::Error),
    #[error("Could not open locks dir")]
    OpenDir(#[source] OsError),
    #[error("Could not create lock file")]
    CreateLockFile(#[source] OsError),
    #[error("Could not lock lock file")]
    LockLockFile(#[source] OsError),
    #[error("Could not link lock file")]
    LinkLockFile(#[source] OsError),
    #[error("Could not read dir")]
    ReadDir(#[source] OsError),
}

type LockFileName = ArrayString<OPAQUE_LEN>;

struct LockFile {
    dir: OwnedFd,
    name: LockFileName,
    _file: OwnedFd,
}

impl LockFile {
    fn new() -> Result<Self, LockFileError> {
        let dir = data_dir().join("db/locks");
        std::fs::create_dir_all(&dir).map_err(LockFileError::CreateDir)?;
        let dir = uapi::openat(AT_FDCWD, &*dir, O_DIRECTORY | O_PATH | O_CLOEXEC, 0)
            .map_os_err(LockFileError::OpenDir)?;
        let file = uapi::openat(dir.raw(), ".", O_TMPFILE | O_RDWR | O_CLOEXEC, 0o644)
            .map_os_err(LockFileError::CreateLockFile)?;
        uapi::flock(file.raw(), LOCK_EX | LOCK_NB).map_os_err(LockFileError::LockLockFile)?;
        let name = opaque().to_string();
        uapi::linkat(file.raw(), "", dir.raw(), &*name, AT_EMPTY_PATH)
            .map_os_err(LockFileError::LinkLockFile)?;
        Ok(Self {
            dir,
            name,
            _file: file,
        })
    }

    fn collect_lock_files(&self) -> Result<Vec<LockFileName>, LockFileError> {
        let dir =
            uapi::openat(self.dir.raw(), ".", O_DIRECTORY, 0).map_os_err(LockFileError::OpenDir)?;
        let mut dir = uapi::fdopendir(dir).map_os_err(LockFileError::OpenDir)?;
        let mut res = Vec::new();
        while let Some(entry) = uapi::readdir(&mut dir) {
            let entry = entry.map_os_err(LockFileError::ReadDir)?;
            if entry.d_type != c::DT_REG {
                continue;
            }
            let Ok(name) = entry.name().to_str() else {
                continue;
            };
            if name.len() != OPAQUE_LEN {
                continue;
            }
            let name = LockFileName::from(name).unwrap();
            let Ok(file) = uapi::openat(self.dir.raw(), &*name, O_RDONLY | O_CLOEXEC, 0) else {
                continue;
            };
            if uapi::flock(file.raw(), LOCK_EX | LOCK_NB).is_err() {
                res.push(name);
            } else {
                let _ = uapi::unlinkat(self.dir.raw(), &*name, 0);
            }
        }
        Ok(res)
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        if let Err(e) = uapi::unlinkat(self.dir.raw(), &*self.name, 0) {
            log::error!("Could not unlink lock file: {}", ErrorFmt(e));
        }
    }
}

pub async fn handle_sqlite_optimize(state: Rc<State>) {
    let Some(sqlite) = &state.sqlite else {
        return;
    };
    const NSEC_PER_HOUR: u64 = Duration::from_hours(1).as_nanos() as u64;
    loop {
        let res = state.ring.timeout(state.now_nsec() + NSEC_PER_HOUR).await;
        if let Err(e) = res {
            log::error!("Could not sleep for an hour: {}", ErrorFmt(e));
            return;
        }
        sqlite.add_job(None, Box::new(OptimizeJob));
    }
    struct OptimizeJob;
    impl SqliteJob for OptimizeJob {
        fn work(&mut self) -> &mut dyn SqliteWork {
            self
        }

        fn completed(self: Box<Self>) {
            // nothing
        }
    }
    impl SqliteWork for OptimizeJob {
        fn run(&mut self, ctx: &mut SqliteCtx) {
            log::debug!("Running pragma optimize");
            if let Err(e) = ctx.db.exec("pragma optimize") {
                log::error!("Could not optimize database: {}", ErrorFmt(e));
            }
        }
    }
}
