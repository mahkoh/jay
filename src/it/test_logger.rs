use {
    crate::utils::clonecell::CloneCell,
    log::{Level, LevelFilter, Log, Metadata, Record},
    std::{
        fmt::Write as FmtWrite,
        io::Write,
        rc::Rc,
        sync::atomic::{AtomicUsize, Ordering},
        time::SystemTime,
    },
    uapi::{Fd, OwnedFd},
};

static LEVEL: AtomicUsize = AtomicUsize::new(Level::Info as usize);

#[thread_local]
static FILE: CloneCell<Option<Rc<OwnedFd>>> = CloneCell::new(None);

pub fn install() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(LevelFilter::Info);
}

pub fn set_level(level: Level) {
    LEVEL.store(level as usize, Ordering::Relaxed);
    log::set_max_level(level.to_level_filter());
}

pub fn set_file(file: Rc<OwnedFd>) {
    FILE.set(Some(file));
}

pub fn unset_file() {
    FILE.set(None);
}

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() as usize <= LEVEL.load(Ordering::Relaxed)
    }

    fn log(&self, record: &Record) {
        if record.level() as usize > LEVEL.load(Ordering::Relaxed) {
            return;
        }
        let mut buf = String::new();
        let now = SystemTime::now();
        let _ = if let Some(mp) = record.module_path() {
            writeln!(
                buf,
                "[{} {:5} {}] {}",
                humantime::format_rfc3339_millis(now),
                record.level(),
                mp,
                record.args(),
            )
        } else {
            writeln!(
                buf,
                "[{} {:5}] {}",
                humantime::format_rfc3339_millis(now),
                record.level(),
                record.args(),
            )
        };
        let mut fd = match FILE.get() {
            Some(f) => f.borrow(),
            _ => Fd::new(2),
        };
        let _ = fd.write_all(buf.as_bytes());
    }

    fn flush(&self) {
        // nothing
    }
}
