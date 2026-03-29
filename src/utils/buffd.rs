use {crate::io_uring::IoUringError, thiserror::Error};
pub use {
    buf_in::BufFdIn,
    buf_out::{BufFdOut, OutBuffer, OutBufferSwapchain},
    ei_formatter::EiMsgFormatter,
    ei_parser::{EiMsgParser, EiMsgParserError},
    formatter::MsgFormatter,
    parser::{MsgParser, MsgParserError},
    wl_buf_in::{WlBufFdIn, WlMessage},
};

mod buf_in;
mod buf_out;
mod ei_formatter;
mod ei_parser;
mod formatter;
mod parser;
mod wl_buf_in;

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
    #[error("Message size is not a multiple of 4")]
    UnalignedMessageSize,
    #[error("Message size is larger than 4096")]
    MessageTooLarge,
    #[error("Message size is smaller than 8")]
    MessageTooSmall,
}

const BUF_SIZE: usize = 4096;
const MAX_IN_FD: usize = 32;
