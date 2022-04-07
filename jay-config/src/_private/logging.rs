use {
    crate::LogLevel,
    log::{Level, LevelFilter, Log, Metadata, Record},
};

pub fn init() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(LevelFilter::Trace);
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
