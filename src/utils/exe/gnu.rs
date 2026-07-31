use crate::utils::exe::ExeError;
use crate::utils::oserror::OsErrorExt2;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ptr;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::Relaxed;
use uapi::OwnedFd;
use uapi::c;
use uapi::map_err;

static ARGV: AtomicPtr<*mut c_char> = AtomicPtr::new(ptr::null_mut());
static ENVP: AtomicPtr<*mut c_char> = AtomicPtr::new(ptr::null_mut());

#[used]
#[unsafe(link_section = ".init_array")]
static INIT_ENV: extern "C" fn(c_int, *mut *mut c_char, *mut *mut c_char) = {
    extern "C" fn init(_argc: c_int, argv: *mut *mut c_char, envp: *mut *mut c_char) {
        ARGV.store(argv, Relaxed);
        ENVP.store(envp, Relaxed);
    }
    init
};

pub fn reexec(target: &OwnedFd) -> Result<(), ExeError> {
    let res = unsafe {
        c::execveat(
            target.raw(),
            c"".as_ptr(),
            ARGV.load(Relaxed),
            ENVP.load(Relaxed),
            c::AT_EMPTY_PATH,
        )
    };
    map_err!(res).map_os_err(ExeError::Exec).map(drop)
}
