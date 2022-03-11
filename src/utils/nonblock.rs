use crate::utils::oserror::OsError;
use uapi::c;

pub fn set_nonblock(fd: c::c_int) -> Result<(), OsError> {
    let fl = uapi::fcntl_getfl(fd)?;
    uapi::fcntl_setfl(fd, fl | c::SOCK_NONBLOCK)?;
    Ok(())
}
