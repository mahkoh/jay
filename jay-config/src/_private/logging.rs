use {
    crate::logging::LogLevel,
    backtrace::Backtrace,
    log::{Level, LevelFilter, Log, Metadata, Record},
};

pub fn init() {
    let _ = log::set_logger(&Logger);
    log::set_max_level(LevelFilter::Trace);
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

struct Logger;

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let client = get!();
        let level = match record.level() {
            Level::Error => LogLevel::Error,
            Level::Warn => LogLevel::Warn,
            Level::Info => LogLevel::Info,
            Level::Debug => LogLevel::Debug,
            Level::Trace => LogLevel::Trace,
        };
        let formatted;
        let msg = match record.args().as_str() {
            Some(s) => s,
            _ => {
                formatted = record.args().to_string();
                &formatted
            }
        };
        client.log(level, msg, record.file(), record.line());
    }

    fn flush(&self) {
        // nothing
    }
}
