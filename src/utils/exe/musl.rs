use crate::utils::exe::ExeError;
use jay_algorithms::oserror::OsError;
use uapi::OwnedFd;
use uapi::c;

pub fn reexec(_target: &OwnedFd) -> Result<(), ExeError> {
    Err(ExeError::Exec(OsError(c::EOPNOTSUPP)))
}
