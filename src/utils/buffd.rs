use {crate::io_uring::IoUringError, thiserror::Error};
pub use {
    buf_in::BufFdIn,
    buf_out::{BufFdOut, OutBuffer, OutBufferSwapchain},
    formatter::MsgFormatter,
    parser::{MsgParser, MsgParserError},
};

mod buf_in;
mod buf_out;
mod formatter;
mod parser;

#[derive(Debug, Error)]
pub enum BufFdError {
    #[error("An IO error occurred")]
    Io(#[source] IoUringError),
    #[error("An io-uring error occurred")]
    Ring(#[from] IoUringError),
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
const MAX_IN_FD: usize = 32;
