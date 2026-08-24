use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt;
use std::sync::atomic::AtomicU32;
use uapi::c;
use uapi::map_err;

#[expect(unused)]
pub fn futex_wait(v: &AtomicU32, e: u32) -> Result<(), OsError> {
    let res = unsafe {
        c::syscall(
            c::SYS_futex,
            v.as_ptr().expose_provenance(),
            (c::FUTEX_WAIT | c::FUTEX_PRIVATE_FLAG) as usize,
            e as usize,
            0 as usize,
        )
    };
    map_err!(res).map(drop).to_os_error()
}

#[expect(unused)]
pub fn futex_wake(v: &AtomicU32) -> Result<(), OsError> {
    let res = unsafe {
        c::syscall(
            c::SYS_futex,
            v.as_ptr().expose_provenance(),
            (c::FUTEX_WAKE | c::FUTEX_PRIVATE_FLAG) as usize,
            c::INT_MAX as usize,
        )
    };
    map_err!(res).map(drop).to_os_error()
}
