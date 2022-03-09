use crate::async_engine::AsyncError;
pub use buf_in::BufFdIn;
pub use buf_out::{BufFdOut, OutBufferSwapchain};
pub use formatter::MsgFormatter;
pub use parser::{MsgParser, MsgParserError};
use thiserror::Error;

mod buf_in;
mod buf_out;
mod formatter;
mod parser;

#[derive(Debug, Error)]
pub enum BufFdError {
    #[error("An IO error occurred")]
    Io(#[source] crate::utils::oserror::OsError),
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
const MAX_IN_FD: usize = 32;
