//! Tools for modifying the logging behavior of the compositor.
//!
//! Note that you can use the `log` crate for logging. All invocations of `log::info` etc.
//! automatically log into the compositors log.

use serde::{Deserialize, Serialize};

/// The log level of the compositor or a log message.
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
