use {
    crate::utils::{errorfmt::ErrorFmt, oserror::OsError},
    backtrace::Backtrace,
    bstr::BString,
    log::{Level, Log, Metadata, Record},
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
        time::SystemTime,
    },
    uapi::{Errno, Fd, OwnedFd, Ustring, c, format_ustr},
};

thread_local! {
    static BUFFER: Cell<*mut Vec<u8>> = const { Cell::new(ptr::null_mut()) };
}

pub struct Logger {
    level: AtomicU32,
    path: Mutex<Arc<BString>>,
    _file: Mutex<OwnedFd>,
    file_fd: AtomicI32,
}

impl Logger {
    pub fn install_stderr(level: Level) -> Arc<Self> {
        let file = match uapi::fcntl_dupfd_cloexec(2, 0) {
            Ok(fd) => fd,
            Err(e) => {
                let e = OsError::from(e);
                fatal!("Error: Could not dup stderr: {}", ErrorFmt(e));
            }
        };
        Self::install(level, b"STDERR", file)
    }

    pub fn install_compositor(level: Level) -> Arc<Self> {
        let (path, file) = open_log_file("jay");
        Self::install(level, path.as_bytes(), file)
    }

    pub fn install_pipe(file: OwnedFd, level: Level) -> Arc<Self> {
        Self::install(level, b"PIPE", file)
    }

    fn install(level: Level, path: &[u8], file: OwnedFd) -> Arc<Self> {
        let slf = Arc::new(Self {
            level: AtomicU32::new(level as _),
            path: Mutex::new(Arc::new(path.to_vec().into())),
            file_fd: AtomicI32::new(file.raw()),
            _file: Mutex::new(file),
        });
        log::set_boxed_logger(Box::new(LogWrapper {
            logger: slf.clone(),
        }))
        .unwrap();
        log::set_max_level(level.to_level_filter());
        set_panic_hook();
        slf
    }

    pub fn set_level(&self, level: Level) {
        self.level.store(level as _, Relaxed);
        log::set_max_level(level.to_level_filter());
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
        ) {
            Ok(f) => return (file_name, f),
            Err(Errno(c::EEXIST)) => {}
            Err(e) => {
                let e: OsError = e.into();
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
        metadata.level() as u32 <= self.logger.level.load(Relaxed)
    }

    fn log(&self, record: &Record) {
        if record.level() as u32 > self.logger.level.load(Relaxed) {
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
