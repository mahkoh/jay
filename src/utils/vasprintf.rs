use std::ffi::VaList;
use uapi::c;

unsafe extern "C" {
    pub fn vasprintf(strp: *mut *mut c::c_char, fmt: *const c::c_char, ap: VaList<'_>) -> c::c_int;
}
