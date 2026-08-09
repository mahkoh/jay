use crate::utils::exe::ExeError;
use crate::utils::oserror::OsError;
use uapi::OwnedFd;
use uapi::c;

pub fn reexec(_target: &OwnedFd) -> Result<(), ExeError> {
    Err(ExeError::Exec(OsError(c::EOPNOTSUPP)))
}
