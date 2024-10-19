use {crate::utils::oserror::OsError, uapi::c};

pub fn set_nonblock(fd: c::c_int) -> Result<(), OsError> {
    let fl = uapi::fcntl_getfl(fd)?;
    uapi::fcntl_setfl(fd, fl | c::O_NONBLOCK)?;
    Ok(())
}

pub fn set_block(fd: c::c_int) -> Result<(), OsError> {
    let fl = uapi::fcntl_getfl(fd)?;
    uapi::fcntl_setfl(fd, fl & !c::O_NONBLOCK)?;
    Ok(())
}
