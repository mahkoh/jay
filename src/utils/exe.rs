use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c::O_CLOEXEC;
use uapi::c::O_PATH;

#[cfg_attr(target_env = "gnu", path = "./exe/gnu.rs")]
#[cfg_attr(target_env = "musl", path = "./exe/musl.rs")]
mod reexec;

pub fn open_exe() -> Result<Rc<OwnedFd>, OsError> {
    uapi::open("/proc/self/exe", O_PATH | O_CLOEXEC, 0)
        .to_os_error()
        .map(Rc::new)
}

pub fn is_same_ino(left: &OwnedFd, right: &OwnedFd) -> Result<bool, OsError> {
    let left = uapi::fstat(left.raw()).to_os_error()?;
    let right = uapi::fstat(right.raw()).to_os_error()?;
    Ok((left.st_dev, left.st_ino) == (right.st_dev, right.st_ino))
}

#[derive(Debug, Error)]
pub enum ExeError {
    #[error("Could not open /proc/self/exe")]
    OpenExe(#[source] OsError),
    #[error("Could not compare inodes")]
    CheckInodes(#[source] OsError),
    #[error("Could not exec target fd")]
    Exec(#[source] OsError),
}

pub fn ensure_same_exe(target: &OwnedFd) -> Result<(), ExeError> {
    let slf = open_exe().map_err(ExeError::OpenExe)?;
    if is_same_ino(&slf, target).map_err(ExeError::CheckInodes)? {
        return Ok(());
    }
    log::warn!("Reexecing to match binary version");
    reexec::reexec(target)
}
