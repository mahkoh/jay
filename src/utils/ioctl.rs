use {
    crate::utils::{compat::IoctlNumber, oserror::OsError},
    uapi::c,
};

pub unsafe fn ioctl<T>(fd: c::c_int, request: c::c_ulong, t: &mut T) -> Result<c::c_int, OsError> {
    let mut ret;
    loop {
        ret = unsafe { c::ioctl(fd, request as IoctlNumber, &mut *t) };
        if ret != -1 {
            return Ok(ret);
        }
        let err = uapi::get_errno();
        if not_matches!(err, c::EINTR | c::EAGAIN) {
            return Err(OsError(err));
        }
    }
}
