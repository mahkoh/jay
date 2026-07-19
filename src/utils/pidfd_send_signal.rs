use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt;
use c::c_int;
use c::syscall;
use std::ptr;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c::SYS_pidfd_send_signal;
use uapi::c::siginfo_t;
use uapi::c::{self};
use uapi::map_err;

pub fn pidfd_send_signal(pidfd: &Rc<OwnedFd>, signal: c_int) -> Result<(), OsError> {
    let res = unsafe {
        syscall(
            SYS_pidfd_send_signal,
            pidfd.raw(),
            signal,
            ptr::null_mut::<siginfo_t>(),
            0,
        )
    };
    map_err!(res).map(drop).to_os_error()
}
