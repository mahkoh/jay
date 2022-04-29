use {
    crate::utils::{errorfmt::ErrorFmt, oserror::OsError},
    backtrace::Backtrace,
    bstr::{BStr, BString, ByteSlice},
    log::{Level, Log, Metadata, Record},
    std::{
        fs::DirBuilder,
        io::Write,
        os::unix::{ffi::OsStringExt, fs::DirBuilderExt},
        sync::{
            atomic::{AtomicU32, Ordering::Relaxed},
            Arc,
        },
        time::SystemTime,
    },
    uapi::{c, format_ustr, Errno, Fd, OwnedFd},
};

#[thread_local]
static mut BUFFER: Vec<u8> = Vec::new();

pub struct Logger {
    level: AtomicU32,
    path: BString,
    file: OwnedFd,
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
        Self::install(level, b"", file)
    }

    pub fn install_compositor(level: Level) -> Arc<Self> {
        let log_dir = create_log_dir();
        let (path, file) = 'file: {
            for i in 0.. {
                let file_name = format_ustr!(
                    "{}/jay-{}-{}.txt",
                    log_dir,
                    humantime::format_rfc3339_millis(SystemTime::now()),
                    i,
                );
                match uapi::open(
                    &file_name,
                    c::O_CREAT | c::O_EXCL | c::O_CLOEXEC | c::O_WRONLY,
                    0o644,
                ) {
                    Ok(f) => break 'file (file_name, f),
                    Err(Errno(c::EEXIST)) => {}
                    Err(e) => {
                        let e: OsError = e.into();
                        fatal!("Error: Could not create log file: {}", ErrorFmt(e));
                    }
                }
            }
            unreachable!();
        };
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
        Self::install(level, path.as_bytes(), file)
    }

    fn install(level: Level, path: &[u8], file: OwnedFd) -> Arc<Self> {
        let slf = Arc::new(Self {
            level: AtomicU32::new(level as _),
            path: path.to_vec().into(),
            file,
        });
        log::set_boxed_logger(Box::new(LogWrapper {
            logger: slf.clone(),
        }))
        .unwrap();
        log::set_max_level(level.to_level_filter());
        slf
    }

    pub fn set_level(&self, level: Level) {
        self.level.store(level as _, Relaxed);
        log::set_max_level(level.to_level_filter());
    }

    pub fn path(&self) -> &BStr {
        self.path.as_bstr()
    }
}

fn create_log_dir() -> BString {
    let mut log_dir = match dirs::data_local_dir() {
        Some(d) => d,
        None => fatal!("Error: $HOME is not set"),
    };
    log_dir.push("jay");
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
        let buffer = unsafe { &mut BUFFER };
        buffer.clear();
        let now = SystemTime::now();
        let _ = if let Some(mp) = record.module_path() {
            writeln!(
                buffer,
                "[{} {:5} {}] {}",
                humantime::format_rfc3339_millis(now),
                record.level(),
                mp,
                record.args(),
            )
        } else {
            writeln!(
                buffer,
                "[{} {:5}] {}",
                humantime::format_rfc3339_millis(now),
                record.level(),
                record.args(),
            )
        };
        let mut fd = Fd::new(self.logger.file.raw());
        let _ = fd.write_all(buffer);
    }

    fn flush(&self) {
        // nothing
    }
}
