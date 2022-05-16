//! Tools for modifying the logging behavior of the compositor.
//!
//! Note that you can use the `log` crate for logging. All invocations of `log::info` etc.
//! automatically log into the compositors log.

use bincode::{Decode, Encode};

/// The log level of the compositor or a log message.
#[derive(Encode, Decode, Copy, Clone, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
