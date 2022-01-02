use crate::async_engine::AsyncError;
pub use buf_in::BufFdIn;
pub use buf_out::BufFdOut;
use thiserror::Error;
pub use wl_formatter::WlFormatter;
pub use wl_parser::{WlParser, WlParserError};

mod buf_in;
mod buf_out;
mod wl_formatter;
mod wl_parser;

#[derive(Debug, Error)]
pub enum BufFdError {
    #[error("An IO error occurred")]
    Io(#[source] std::io::Error),
    #[error("An async error occurred")]
    Async(#[from] AsyncError),
    #[error("The peer did not send a file descriptor")]
    NoFd,
    #[error("The peer sent too many file descriptors")]
    TooManyFds,
    #[error("The peer closed the connection")]
    Closed,
    #[error("The connection timed out")]
    Timeout,
}

const BUF_SIZE: usize = 4096;
const CMSG_BUF_SIZE: usize = 4096;
const MAX_IN_FD: usize = 4;
