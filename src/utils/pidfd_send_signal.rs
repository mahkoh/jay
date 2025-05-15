use {
    crate::utils::oserror::OsError,
    c::{c_int, syscall},
    std::{ptr, rc::Rc},
    uapi::{
        OwnedFd,
        c::{self, SYS_pidfd_send_signal, siginfo_t},
        map_err,
    },
};

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
    map_err!(res).map(drop).map_err(|e| e.into())
}
