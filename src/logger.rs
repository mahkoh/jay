use {
    crate::{
        compositor::LogLevel,
        utils::{
            atomic_enum::AtomicEnum,
            errorfmt::ErrorFmt,
            oserror::{OsError, OsErrorExt, OsErrorExt2},
        },
    },
    backtrace::Backtrace,
    bstr::{BStr, BString, ByteSlice},
    log::{LevelFilter, Log, Metadata, Record},
    parking_lot::Mutex,
    std::{
        cell::Cell,
        fs::DirBuilder,
        io::Write,
        os::unix::{ffi::OsStringExt, fs::DirBuilderExt},
        ptr,
        sync::{
            Arc,
            atomic::{AtomicI32, AtomicU32, Ordering::Relaxed},
        },
        thread,
        time::SystemTime,
    },
    thiserror::Error,
    uapi::{AsUstr, Dirent, Fd, OwnedFd, Ustring, c, format_ustr},
};

thread_local! {
    static BUFFER: Cell<*mut Vec<u8>> = const { Cell::new(ptr::null_mut()) };
}

pub struct Logger {
    level: AtomicEnum<LogLevel>,
    filter: AtomicU32,
    path: Mutex<Arc<BString>>,
    _file: Mutex<OwnedFd>,
    file_fd: AtomicI32,
}

impl Logger {
    pub fn install_stderr(level: LogLevel) -> Arc<Self> {
        let file = match uapi::fcntl_dupfd_cloexec(2, 0).to_os_error() {
            Ok(fd) => fd,
            Err(e) => {
                fatal!("Error: Could not dup stderr: {}", ErrorFmt(e));
            }
        };
        Self::install(level, b"STDERR", file)
    }

    pub fn install_compositor(level: LogLevel) -> Arc<Self> {
        let (path, file) = open_log_file("jay");
        Self::install(level, path.as_bytes(), file)
    }

    pub fn install_pipe(file: OwnedFd, level: LogLevel) -> Arc<Self> {
        Self::install(level, b"PIPE", file)
    }

    fn install(level: LogLevel, path: &[u8], file: OwnedFd) -> Arc<Self> {
        let filter: LevelFilter = level.into();
        let slf = Arc::new(Self {
            level: AtomicEnum::new(level),
            filter: AtomicU32::new(filter as _),
            path: Mutex::new(Arc::new(path.to_vec().into())),
            file_fd: AtomicI32::new(file.raw()),
            _file: Mutex::new(file),
        });
        log::set_boxed_logger(Box::new(LogWrapper {
            logger: slf.clone(),
        }))
        .unwrap();
        log::set_max_level(filter);
        set_panic_hook();
        slf
    }

    pub fn set_level(&self, level: LogLevel) {
        let filter: LevelFilter = level.into();
        self.level.store(level, Relaxed);
        self.filter.store(filter as _, Relaxed);
        log::set_max_level(filter);
    }

    pub fn clean_logs_older_than(&self, time: SystemTime) {
        let time_formatted = humantime::format_rfc3339_millis(time);
        log::info!("Cleaning unused log files older than {}", time_formatted);
        let path = self.path();
        thread::spawn(move || {
            if let Err(e) = clean_logs_older_than(path.as_bstr(), time) {
                log::error!("Could not clean log files: {}", ErrorFmt(e));
            }
        });
    }

    pub fn level(&self) -> LogLevel {
        self.level.load(Relaxed)
    }

    pub fn path(&self) -> Arc<BString> {
        self.path.lock().clone()
    }

    pub fn redirect(&self, ty: &str) -> Ustring {
        let (file, fd) = open_log_file(ty);
        log::info!("Redirecting logs to {}", file.display());
        *self.path.lock() = Arc::new(file.as_bytes().into());
        self.file_fd.store(fd.raw(), Relaxed);
        *self._file.lock() = fd;
        file
    }

    pub fn write_raw(&self, buf: &[u8]) {
        let mut fd = Fd::new(self.file_fd.load(Relaxed));
        let _ = fd.write_all(buf);
    }
}

pub fn open_log_file(ty: &str) -> (Ustring, OwnedFd) {
    let log_dir = create_log_dir(ty);
    let mut flock_fail_count = 0;
    for i in 0.. {
        let file_name = format_ustr!(
            "{}/{ty}-{}-{}.txt",
            log_dir,
            humantime::format_rfc3339_millis(SystemTime::now()),
            i,
        );
        match uapi::open(
            &file_name,
            c::O_CREAT | c::O_EXCL | c::O_CLOEXEC | c::O_WRONLY,
            0o644,
        )
        .to_os_error()
        {
            Ok(f) => {
                if let Err(e) = uapi::flock(f.raw(), c::LOCK_EX | c::LOCK_NB) {
                    log::warn!("Unable to flock just-opened logfile: {}", ErrorFmt(e));
                    flock_fail_count += 1;
                    if flock_fail_count > 10 {
                        log::error!(concat!(
                            "Failed to flock just-opened logfile more than 10 times in a row. ",
                            "Not flocking the logfile, if the cleanup routine later succeeds to ",
                            "flock this logfile, it will be deleted even if it is still in use."
                        ));
                    } else {
                        continue;
                    }
                }
                return (file_name, f);
            }
            Err(OsError(c::EEXIST)) => {}
            Err(e) => {
                fatal!("Error: Could not create log file: {}", ErrorFmt(e));
            }
        }
    }
    unreachable!()
}

fn create_log_dir(ty: &str) -> BString {
    let mut log_dir = match dirs::data_local_dir() {
        Some(d) => d,
        None => fatal!("Error: $HOME is not set"),
    };
    log_dir.push("jay");
    log_dir.push("logs");
    log_dir.push(ty);
    let res = DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(&log_dir);
    if let Err(e) = res {
        fatal!(
            "Error: Could not create log directory {}: {}",
            log_dir.display(),
            ErrorFmt(e)
        );
    }
    log_dir.into_os_string().into_vec().into()
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|p| {
        if let Some(loc) = p.location() {
            log::error!(
                "Panic at {} line {} column {}",
                loc.file(),
                loc.line(),
                loc.column()
            );
        } else {
            log::error!("Panic at unknown location");
        }
        if let Some(msg) = p.payload().downcast_ref::<&str>() {
            log::error!("Message: {}", msg);
        }
        if let Some(msg) = p.payload().downcast_ref::<String>() {
            log::error!("Message: {}", msg);
        }
        log::error!("Backtrace:\n{:?}", Backtrace::new());
    }));
}

struct LogWrapper {
    logger: Arc<Logger>,
}

impl Log for LogWrapper {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() as u32 <= self.logger.filter.load(Relaxed)
    }

    fn log(&self, record: &Record) {
        if record.level() as u32 > self.logger.filter.load(Relaxed) {
            return;
        }
        let mut buffer = BUFFER.get();
        if buffer.is_null() {
            buffer = Box::into_raw(Box::default());
            BUFFER.set(buffer);
        }
        let buffer = unsafe { &mut *buffer };
        buffer.clear();
        let now = SystemTime::now();
        let _ = writeln!(
            buffer,
            "[{} {:5} {}] {}",
            humantime::format_rfc3339_millis(now),
            record.level(),
            record.target(),
            record.args(),
        );
        let mut fd = Fd::new(self.logger.file_fd.load(Relaxed));
        let _ = fd.write_all(buffer);
    }

    fn flush(&self) {
        // nothing
    }
}

#[derive(Debug, Error)]
enum CleanLogsError {
    #[error("Log path has no parent")]
    NoParent,
    #[error("Could not open the log directory")]
    OpenDir(#[source] OsError),
    #[error("Could not enumerate directory entry")]
    ReadDir(#[source] OsError),
    #[error("Could not open the log file")]
    OpenFile(#[source] OsError),
    #[error("Could not stat the log file")]
    Stat(#[source] OsError),
    #[error("Could not unlink the log file")]
    Unlink(#[source] OsError),
}

fn clean_logs_older_than(current_log_path: &BStr, time: SystemTime) -> Result<(), CleanLogsError> {
    let current_log_path = current_log_path.to_path_lossy();
    let parent = current_log_path.parent().ok_or(CleanLogsError::NoParent)?;
    let mut dir = uapi::opendir(parent).map_os_err(CleanLogsError::OpenDir)?;
    let parent = uapi::open(parent, c::O_PATH | c::O_CLOEXEC | c::O_DIRECTORY, 0)
        .map_os_err(CleanLogsError::OpenDir)?;
    let time = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as c::time_t;
    while let Some(entry) = uapi::readdir(&mut dir) {
        let entry = entry.map_os_err(CleanLogsError::ReadDir)?;
        if let Err(err) = process_entry(parent.raw(), &entry, time) {
            log::error!(
                "Could not clean log file {}: {}",
                entry.name().as_ustr().display(),
                ErrorFmt(err),
            );
        }
    }
    fn process_entry(
        parent: c::c_int,
        entry: &Dirent,
        time: c::time_t,
    ) -> Result<(), CleanLogsError> {
        if entry.d_type != c::DT_REG {
            return Ok(());
        }
        let name = entry.name();
        let file = uapi::openat(parent, name, c::O_RDONLY | c::O_CLOEXEC, 0)
            .map_os_err(CleanLogsError::OpenFile)?;
        let stat = uapi::fstat(*file).map_os_err(CleanLogsError::Stat)?;
        if stat.st_mtime >= time {
            return Ok(());
        }
        if uapi::flock(file.raw(), c::LOCK_EX | c::LOCK_NB).is_err() {
            log::info!("Preserving file still in use: {}", name.as_ustr().display());
            return Ok(());
        }
        uapi::unlinkat(parent, name, 0).map_os_err(CleanLogsError::Unlink)?;
        log::info!("Deleted {}", name.as_ustr().display());
        Ok(())
    }
    Ok(())
}
