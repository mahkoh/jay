use crate::{io_uring::IoUringError, utils::oserror::OsError};

pub mod async_cancel;
pub mod poll;
pub mod recvmsg;
pub mod sendmsg;
pub mod timeout;
pub mod write;

pub type TaskResult<T> = Result<Result<T, OsError>, IoUringError>;

pub trait TaskResultExt<T> {
    fn merge(self) -> Result<T, IoUringError>;
}

impl<T> TaskResultExt<T> for TaskResult<T> {
    fn merge(self) -> Result<T, IoUringError> {
        match self {
            Ok(Ok(t)) => Ok(t),
            Ok(Err(e)) => Err(IoUringError::OsError(e)),
            Err(e) => Err(e),
        }
    }
}
