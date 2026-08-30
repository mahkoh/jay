use crate::io_uring::IoUringError;
use jay_algorithms::oserror::OsError;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FuseError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The filesystem was unmounted")]
    Aborted,
    #[error("Root inode has been dropped")]
    RootInodeDropped,
    #[error("Root inode must have ID 1")]
    RootInodeId,
    #[error("Root inode must be a directory")]
    RootInodeDir,
    #[error("There is no forker")]
    NoForker,
    #[error("Could not create a socketpair")]
    CreateSocketpair(#[source] OsError),
    #[error("Could not create a pipe")]
    CreatePipe(#[source] OsError),
    #[error("Recvmsg failed")]
    Recvmsg(#[source] IoUringError),
    #[error("fusermount3 did not send an fd")]
    NoFd,
    #[error("Could not create a buffer ring")]
    CreateBufferRing(#[source] IoUringError),
    #[error("Could not schedule request reading")]
    ScheduleReadRequest(#[source] IoUringError),
    #[error("Could not read requests")]
    ReadRequests(#[source] OsError),
    #[error("Could not spawn prune thread")]
    SpawnPruneThread(#[source] io::Error),
    #[error("Could not sleep")]
    Sleep(#[source] IoUringError),
    #[error("FUTEX_WAKE failed")]
    FutexWake(#[source] IoUringError),
    #[error("FUTEX_WAIT failed")]
    FutexWait(#[source] IoUringError),
    #[error("Root name must not contain null bytes")]
    InvalidRootName,
    #[error("Could not create memfd")]
    CreateMemfd(#[source] OsError),
    #[error("Could not write to memfd")]
    WriteMemfd(#[source] io::Error),
    #[error("Could not poll pipe")]
    Poll(#[source] IoUringError),
}
