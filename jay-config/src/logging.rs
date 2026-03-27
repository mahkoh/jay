//! Tools for modifying the logging behavior of the compositor.
//!
//! Note that you can use the `log` crate for logging. All invocations of `log::info` etc.
//! automatically log into the compositors log.

use {
    serde::{Deserialize, Serialize},
    std::time::SystemTime,
};

/// The log level of the compositor or a log message.
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Sets the log level of the compositor.
pub fn set_log_level(level: LogLevel) {
    get!().set_log_level(level);
}

/// If this function is called during startup, Jay's log files before `time` are deleted.
///
/// The current log file is never deleted, nor are any other logfiles of active Jay instances (e.g.
/// on another VT), even if `time` is in the future.
///
/// Calling this function after startup has no effect.
pub fn clean_logs_older_than(time: SystemTime) {
    get!().clean_logs_older_than(time);
}
