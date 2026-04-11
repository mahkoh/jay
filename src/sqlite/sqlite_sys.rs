#![expect(non_camel_case_types)]

use std::{
    error::Error,
    ffi::{c_char, c_int, c_uint, c_void},
    fmt::{Display, Formatter},
};

pub struct sqlite3(#[expect(dead_code)] u8);
pub struct sqlite3_stmt(#[expect(dead_code)] u8);

dynload! {
    SQLITE: Sqlite from "libsqlite3.so" {
        sqlite3_bind_blob: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            arg2: c_int,
            arg3: *const c_void,
            n: c_int,
            arg4: sqlite3_destructor_type,
        ) -> SqliteResult,
        sqlite3_bind_int64: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            arg2: c_int,
            arg3: i64,
        ) -> SqliteResult,
        // sqlite3_busy_timeout: unsafe extern "C" fn(arg1: *mut sqlite3, ms: c_int) -> c_int,
        sqlite3_clear_bindings: unsafe extern "C" fn(arg1: *mut sqlite3_stmt) -> SqliteResult,
        sqlite3_close: unsafe extern "C" fn(arg1: *mut sqlite3) -> SqliteResult,
        sqlite3_column_blob: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            iCol: c_int,
        ) -> *const c_void,
        sqlite3_column_bytes: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            iCol: c_int,
        ) -> c_int,
        sqlite3_column_int64: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            iCol: c_int,
        ) -> i64,
        sqlite3_column_type: unsafe extern "C" fn(
            arg1: *mut sqlite3_stmt,
            iCol: c_int,
        ) -> c_int,
        sqlite3_config: unsafe extern "C" fn(arg1: c_int, ...) -> SqliteResult,
        // sqlite3_db_config: unsafe extern "C" fn(arg1: *mut sqlite3, op: c_int, ...) -> SqliteResult,
        sqlite3_exec: unsafe extern "C" fn(
            arg1: *mut sqlite3,
            sql: *const c_char,
            callback: Option<
                unsafe extern "C" fn(
                    arg1: *mut c_void,
                    arg2: c_int,
                    arg3: *mut *mut c_char,
                    arg4: *mut *mut c_char,
                ) -> c_int,
            >,
            arg2: *mut c_void,
            errmsg: *mut *mut c_char,
        ) -> SqliteResult,
        // sqlite3_extended_result_codes: unsafe extern "C" fn(
        //     arg1: *mut sqlite3,
        //     onoff: c_int,
        // ) -> SqliteResult,
        sqlite3_finalize: unsafe extern "C" fn(pStmt: *mut sqlite3_stmt) -> SqliteResult,
        // sqlite3_last_insert_rowid: unsafe extern "C" fn(arg1: *mut sqlite3) -> i64,
        sqlite3_open_v2: unsafe extern "C" fn(
            filename: *const c_char,
            ppDb: *mut *mut sqlite3,
            flags: c_int,
            zVfs: *const c_char,
        ) -> SqliteResult,
        // sqlite3_progress_handler: unsafe extern "C" fn(
        //     arg1: *mut sqlite3,
        //     arg2: c_int,
        //     arg3: Option<unsafe extern "C" fn(arg1: *mut c_void) -> c_int>,
        //     arg4: *mut c_void,
        // ),
        sqlite3_prepare_v3: unsafe extern "C" fn(
            db: *mut sqlite3,
            zSql: *const c_char,
            nByte: c_int,
            prepFlags: c_uint,
            ppStmt: *mut *mut sqlite3_stmt,
            pzTail: *mut *const c_char,
        ) -> SqliteResult,
        sqlite3_reset: unsafe extern "C" fn(pStmt: *mut sqlite3_stmt) -> SqliteResult,
        sqlite3_step: unsafe extern "C" fn(arg1: *mut sqlite3_stmt) -> SqliteResult,
    }
}

macro_rules! sqlite_result {
    (
        @basic:
            $($bcode:ident = $bvalue:expr,)*
        @extended:
            $($ecode:ident = $evalue:expr,)*
    ) => {
        $(pub const $bcode: c_int = $bvalue;)*
        $(pub const $ecode: c_int = $evalue;)*

        impl Display for SqliteResult {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                'extended: {
                    let s = match self.0 {
                        $($ecode => stringify!($ecode),)*
                        _ => break 'extended,
                    };
                    return f.write_str(s);
                }
                let s = match self.0 & 0xff {
                    $($bcode => stringify!($bcode),)*
                    _ => return write!(f, "0x{:x}", self.0),
                };
                f.write_str(s)?;
                if self.0 & !0xff != 0 {
                    write!(f, " (0x{:x})", self.0)?;
                }
                Ok(())
            }
        }
    };
}

sqlite_result! {
    @basic:
        SQLITE_OK = 0,
        SQLITE_ERROR = 1,
        SQLITE_INTERNAL = 2,
        SQLITE_PERM = 3,
        SQLITE_ABORT = 4,
        SQLITE_BUSY = 5,
        SQLITE_LOCKED = 6,
        SQLITE_NOMEM = 7,
        SQLITE_READONLY = 8,
        SQLITE_INTERRUPT = 9,
        SQLITE_IOERR = 10,
        SQLITE_CORRUPT = 11,
        SQLITE_NOTFOUND = 12,
        SQLITE_FULL = 13,
        SQLITE_CANTOPEN = 14,
        SQLITE_PROTOCOL = 15,
        SQLITE_EMPTY = 16,
        SQLITE_SCHEMA = 17,
        SQLITE_TOOBIG = 18,
        SQLITE_CONSTRAINT = 19,
        SQLITE_MISMATCH = 20,
        SQLITE_MISUSE = 21,
        SQLITE_NOLFS = 22,
        SQLITE_AUTH = 23,
        SQLITE_FORMAT = 24,
        SQLITE_RANGE = 25,
        SQLITE_NOTADB = 26,
        SQLITE_NOTICE = 27,
        SQLITE_WARNING = 28,
        SQLITE_ROW = 100,
        SQLITE_DONE = 101,
    @extended:
        SQLITE_ERROR_MISSING_COLLSEQ = SQLITE_ERROR | (1 << 8),
        SQLITE_ERROR_RETRY = SQLITE_ERROR | (2 << 8),
        SQLITE_ERROR_SNAPSHOT = SQLITE_ERROR | (3 << 8),
        SQLITE_ERROR_RESERVESIZE = SQLITE_ERROR | (4 << 8),
        SQLITE_ERROR_KEY = SQLITE_ERROR | (5 << 8),
        SQLITE_ERROR_UNABLE = SQLITE_ERROR | (6 << 8),
        SQLITE_IOERR_READ = SQLITE_IOERR | (1 << 8),
        SQLITE_IOERR_SHORT_READ = SQLITE_IOERR | (2 << 8),
        SQLITE_IOERR_WRITE = SQLITE_IOERR | (3 << 8),
        SQLITE_IOERR_FSYNC = SQLITE_IOERR | (4 << 8),
        SQLITE_IOERR_DIR_FSYNC = SQLITE_IOERR | (5 << 8),
        SQLITE_IOERR_TRUNCATE = SQLITE_IOERR | (6 << 8),
        SQLITE_IOERR_FSTAT = SQLITE_IOERR | (7 << 8),
        SQLITE_IOERR_UNLOCK = SQLITE_IOERR | (8 << 8),
        SQLITE_IOERR_RDLOCK = SQLITE_IOERR | (9 << 8),
        SQLITE_IOERR_DELETE = SQLITE_IOERR | (10 << 8),
        SQLITE_IOERR_BLOCKED = SQLITE_IOERR | (11 << 8),
        SQLITE_IOERR_NOMEM = SQLITE_IOERR | (12 << 8),
        SQLITE_IOERR_ACCESS = SQLITE_IOERR | (13 << 8),
        SQLITE_IOERR_CHECKRESERVEDLOCK = SQLITE_IOERR | (14 << 8),
        SQLITE_IOERR_LOCK = SQLITE_IOERR | (15 << 8),
        SQLITE_IOERR_CLOSE = SQLITE_IOERR | (16 << 8),
        SQLITE_IOERR_DIR_CLOSE = SQLITE_IOERR | (17 << 8),
        SQLITE_IOERR_SHMOPEN = SQLITE_IOERR | (18 << 8),
        SQLITE_IOERR_SHMSIZE = SQLITE_IOERR | (19 << 8),
        SQLITE_IOERR_SHMLOCK = SQLITE_IOERR | (20 << 8),
        SQLITE_IOERR_SHMMAP = SQLITE_IOERR | (21 << 8),
        SQLITE_IOERR_SEEK = SQLITE_IOERR | (22 << 8),
        SQLITE_IOERR_DELETE_NOENT = SQLITE_IOERR | (23 << 8),
        SQLITE_IOERR_MMAP = SQLITE_IOERR | (24 << 8),
        SQLITE_IOERR_GETTEMPPATH = SQLITE_IOERR | (25 << 8),
        SQLITE_IOERR_CONVPATH = SQLITE_IOERR | (26 << 8),
        SQLITE_IOERR_VNODE = SQLITE_IOERR | (27 << 8),
        SQLITE_IOERR_AUTH = SQLITE_IOERR | (28 << 8),
        SQLITE_IOERR_BEGIN_ATOMIC = SQLITE_IOERR | (29 << 8),
        SQLITE_IOERR_COMMIT_ATOMIC = SQLITE_IOERR | (30 << 8),
        SQLITE_IOERR_ROLLBACK_ATOMIC = SQLITE_IOERR | (31 << 8),
        SQLITE_IOERR_DATA = SQLITE_IOERR | (32 << 8),
        SQLITE_IOERR_CORRUPTFS = SQLITE_IOERR | (33 << 8),
        SQLITE_IOERR_IN_PAGE = SQLITE_IOERR | (34 << 8),
        SQLITE_IOERR_BADKEY = SQLITE_IOERR | (35 << 8),
        SQLITE_IOERR_CODEC = SQLITE_IOERR | (36 << 8),
        SQLITE_LOCKED_SHAREDCACHE = SQLITE_LOCKED | (1 << 8),
        SQLITE_LOCKED_VTAB = SQLITE_LOCKED | (2 << 8),
        SQLITE_BUSY_RECOVERY = SQLITE_BUSY | (1 << 8),
        SQLITE_BUSY_SNAPSHOT = SQLITE_BUSY | (2 << 8),
        SQLITE_BUSY_TIMEOUT = SQLITE_BUSY | (3 << 8),
        SQLITE_CANTOPEN_NOTEMPDIR = SQLITE_CANTOPEN | (1 << 8),
        SQLITE_CANTOPEN_ISDIR = SQLITE_CANTOPEN | (2 << 8),
        SQLITE_CANTOPEN_FULLPATH = SQLITE_CANTOPEN | (3 << 8),
        SQLITE_CANTOPEN_CONVPATH = SQLITE_CANTOPEN | (4 << 8),
        SQLITE_CANTOPEN_DIRTYWAL = SQLITE_CANTOPEN | (5 << 8),
        SQLITE_CANTOPEN_SYMLINK = SQLITE_CANTOPEN | (6 << 8),
        SQLITE_CORRUPT_VTAB = SQLITE_CORRUPT | (1 << 8),
        SQLITE_CORRUPT_SEQUENCE = SQLITE_CORRUPT | (2 << 8),
        SQLITE_CORRUPT_INDEX = SQLITE_CORRUPT | (3 << 8),
        SQLITE_READONLY_RECOVERY = SQLITE_READONLY | (1 << 8),
        SQLITE_READONLY_CANTLOCK = SQLITE_READONLY | (2 << 8),
        SQLITE_READONLY_ROLLBACK = SQLITE_READONLY | (3 << 8),
        SQLITE_READONLY_DBMOVED = SQLITE_READONLY | (4 << 8),
        SQLITE_READONLY_CANTINIT = SQLITE_READONLY | (5 << 8),
        SQLITE_READONLY_DIRECTORY = SQLITE_READONLY | (6 << 8),
        SQLITE_ABORT_ROLLBACK = SQLITE_ABORT | (2 << 8),
        SQLITE_CONSTRAINT_CHECK = SQLITE_CONSTRAINT | (1 << 8),
        SQLITE_CONSTRAINT_COMMITHOOK = SQLITE_CONSTRAINT | (2 << 8),
        SQLITE_CONSTRAINT_FOREIGNKEY = SQLITE_CONSTRAINT | (3 << 8),
        SQLITE_CONSTRAINT_FUNCTION = SQLITE_CONSTRAINT | (4 << 8),
        SQLITE_CONSTRAINT_NOTNULL = SQLITE_CONSTRAINT | (5 << 8),
        SQLITE_CONSTRAINT_PRIMARYKEY = SQLITE_CONSTRAINT | (6 << 8),
        SQLITE_CONSTRAINT_TRIGGER = SQLITE_CONSTRAINT | (7 << 8),
        SQLITE_CONSTRAINT_UNIQUE = SQLITE_CONSTRAINT | (8 << 8),
        SQLITE_CONSTRAINT_VTAB = SQLITE_CONSTRAINT | (9 << 8),
        SQLITE_CONSTRAINT_ROWID = SQLITE_CONSTRAINT | (10 << 8),
        SQLITE_CONSTRAINT_PINNED = SQLITE_CONSTRAINT | (11 << 8),
        SQLITE_CONSTRAINT_DATATYPE = SQLITE_CONSTRAINT | (12 << 8),
        SQLITE_NOTICE_RECOVER_WAL = SQLITE_NOTICE | (1 << 8),
        SQLITE_NOTICE_RECOVER_ROLLBACK = SQLITE_NOTICE | (2 << 8),
        SQLITE_NOTICE_RBU = SQLITE_NOTICE | (3 << 8),
        SQLITE_WARNING_AUTOINDEX = SQLITE_WARNING | (1 << 8),
        SQLITE_AUTH_USER = SQLITE_AUTH | (1 << 8),
        SQLITE_OK_LOAD_PERMANENTLY = SQLITE_OK | (1 << 8),
        SQLITE_OK_SYMLINK = SQLITE_OK | (2 << 8),
}

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
#[must_use]
pub struct SqliteResult(pub c_int);

impl SqliteResult {
    pub fn is_ok(&self) -> bool {
        self.0 & 0xff == SQLITE_OK
    }

    pub fn result(self) -> Result<(), Self> {
        if self.is_ok() { Ok(()) } else { Err(self) }
    }
}

impl Error for SqliteResult {}

// pub const SQLITE_CONFIG_SINGLETHREAD: c_int = 1;
// pub const SQLITE_CONFIG_MULTITHREAD: c_int = 2;
// pub const SQLITE_CONFIG_SERIALIZED: c_int = 3;
// pub const SQLITE_CONFIG_MALLOC: c_int = 4;
// pub const SQLITE_CONFIG_GETMALLOC: c_int = 5;
// pub const SQLITE_CONFIG_SCRATCH: c_int = 6;
// pub const SQLITE_CONFIG_PAGECACHE: c_int = 7;
// pub const SQLITE_CONFIG_HEAP: c_int = 8;
// pub const SQLITE_CONFIG_MEMSTATUS: c_int = 9;
// pub const SQLITE_CONFIG_MUTEX: c_int = 10;
// pub const SQLITE_CONFIG_GETMUTEX: c_int = 11;
// pub const SQLITE_CONFIG_LOOKASIDE: c_int = 13;
// pub const SQLITE_CONFIG_PCACHE: c_int = 14;
// pub const SQLITE_CONFIG_GETPCACHE: c_int = 15;
pub const SQLITE_CONFIG_LOG: c_int = 16;
// pub const SQLITE_CONFIG_URI: c_int = 17;
// pub const SQLITE_CONFIG_PCACHE2: c_int = 18;
// pub const SQLITE_CONFIG_GETPCACHE2: c_int = 19;
// pub const SQLITE_CONFIG_COVERING_INDEX_SCAN: c_int = 20;
// pub const SQLITE_CONFIG_SQLLOG: c_int = 21;
// pub const SQLITE_CONFIG_MMAP_SIZE: c_int = 22;
// pub const SQLITE_CONFIG_WIN32_HEAPSIZE: c_int = 23;
// pub const SQLITE_CONFIG_PCACHE_HDRSZ: c_int = 24;
// pub const SQLITE_CONFIG_PMASZ: c_int = 25;
// pub const SQLITE_CONFIG_STMTJRNL_SPILL: c_int = 26;
// pub const SQLITE_CONFIG_SMALL_MALLOC: c_int = 27;
// pub const SQLITE_CONFIG_SORTERREF_SIZE: c_int = 28;
// pub const SQLITE_CONFIG_MEMDB_MAXSIZE: c_int = 29;
// pub const SQLITE_CONFIG_ROWID_IN_VIEW: c_int = 30;

// pub const SQLITE_OPEN_READONLY: c_int = 0x00000001;
pub const SQLITE_OPEN_READWRITE: c_int = 0x00000002;
pub const SQLITE_OPEN_CREATE: c_int = 0x00000004;
// pub const SQLITE_OPEN_DELETEONCLOSE: c_int = 0x00000008;
// pub const SQLITE_OPEN_EXCLUSIVE: c_int = 0x00000010;
// pub const SQLITE_OPEN_AUTOPROXY: c_int = 0x00000020;
// pub const SQLITE_OPEN_URI: c_int = 0x00000040;
// pub const SQLITE_OPEN_MEMORY: c_int = 0x00000080;
// pub const SQLITE_OPEN_MAIN_DB: c_int = 0x00000100;
// pub const SQLITE_OPEN_TEMP_DB: c_int = 0x00000200;
// pub const SQLITE_OPEN_TRANSIENT_DB: c_int = 0x00000400;
// pub const SQLITE_OPEN_MAIN_JOURNAL: c_int = 0x00000800;
// pub const SQLITE_OPEN_TEMP_JOURNAL: c_int = 0x00001000;
// pub const SQLITE_OPEN_SUBJOURNAL: c_int = 0x00002000;
// pub const SQLITE_OPEN_SUPER_JOURNAL: c_int = 0x00004000;
pub const SQLITE_OPEN_NOMUTEX: c_int = 0x00008000;
// pub const SQLITE_OPEN_FULLMUTEX: c_int = 0x00010000;
// pub const SQLITE_OPEN_SHAREDCACHE: c_int = 0x00020000;
// pub const SQLITE_OPEN_PRIVATECACHE: c_int = 0x00040000;
// pub const SQLITE_OPEN_WAL: c_int = 0x00080000;
// pub const SQLITE_OPEN_NOFOLLOW: c_int = 0x01000000;
pub const SQLITE_OPEN_EXRESCODE: c_int = 0x02000000;

pub type sqlite3_destructor_type = *const u8;
pub const SQLITE_STATIC: sqlite3_destructor_type = 0 as _;
// pub const SQLITE_TRANSIENT: sqlite3_destructor_type = -1 as _;

// pub const SQLITE_DBCONFIG_MAINDBNAME: c_int = 1000;
// pub const SQLITE_DBCONFIG_LOOKASIDE: c_int = 1001;
// pub const SQLITE_DBCONFIG_ENABLE_FKEY: c_int = 1002;
// pub const SQLITE_DBCONFIG_ENABLE_TRIGGER: c_int = 1003;
// pub const SQLITE_DBCONFIG_ENABLE_FTS3_TOKENIZER: c_int = 1004;
// pub const SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION: c_int = 1005;
// pub const SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE: c_int = 1006;
// pub const SQLITE_DBCONFIG_ENABLE_QPSG: c_int = 1007;
// pub const SQLITE_DBCONFIG_TRIGGER_EQP: c_int = 1008;
// pub const SQLITE_DBCONFIG_RESET_DATABASE: c_int = 1009;
// pub const SQLITE_DBCONFIG_DEFENSIVE: c_int = 1010;
// pub const SQLITE_DBCONFIG_WRITABLE_SCHEMA: c_int = 1011;
// pub const SQLITE_DBCONFIG_LEGACY_ALTER_TABLE: c_int = 1012;
// pub const SQLITE_DBCONFIG_DQS_DML: c_int = 1013;
// pub const SQLITE_DBCONFIG_DQS_DDL: c_int = 1014;
// pub const SQLITE_DBCONFIG_ENABLE_VIEW: c_int = 1015;
// pub const SQLITE_DBCONFIG_LEGACY_FILE_FORMAT: c_int = 1016;
// pub const SQLITE_DBCONFIG_TRUSTED_SCHEMA: c_int = 1017;
// pub const SQLITE_DBCONFIG_STMT_SCANSTATUS: c_int = 1018;
// pub const SQLITE_DBCONFIG_REVERSE_SCANORDER: c_int = 1019;
// pub const SQLITE_DBCONFIG_ENABLE_ATTACH_CREATE: c_int = 1020;
// pub const SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE: c_int = 1021;
// pub const SQLITE_DBCONFIG_ENABLE_COMMENTS: c_int = 1022;
// pub const SQLITE_DBCONFIG_FP_DIGITS: c_int = 1023;

pub const SQLITE_PREPARE_PERSISTENT: c_uint = 0x01;
// pub const SQLITE_PREPARE_NORMALIZE: c_uint = 0x02;
// pub const SQLITE_PREPARE_NO_VTAB: c_uint = 0x04;
// pub const SQLITE_PREPARE_DONT_LOG: c_uint = 0x10;

pub const SQLITE_INTEGER: c_int = 1;
// pub const SQLITE_FLOAT: c_int = 2;
// pub const SQLITE_TEXT: c_int = 3;
pub const SQLITE_BLOB: c_int = 4;
// pub const SQLITE_NULL: c_int = 5;
