use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt;
use uapi::c;

pub fn set_nonblock(fd: c::c_int) -> Result<(), OsError> {
    let fl = uapi::fcntl_getfl(fd).to_os_error()?;
    uapi::fcntl_setfl(fd, fl | c::O_NONBLOCK).to_os_error()?;
    Ok(())
}

pub fn set_block(fd: c::c_int) -> Result<(), OsError> {
    let fl = uapi::fcntl_getfl(fd).to_os_error()?;
    uapi::fcntl_setfl(fd, fl & !c::O_NONBLOCK).to_os_error()?;
    Ok(())
}
