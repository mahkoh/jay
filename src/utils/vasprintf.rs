use std::ffi::{CStr, VaList};
use std::ops::Deref;
use std::ptr;
use uapi::c;

extern "C" {
    fn vasprintf(strp: *mut *mut c::c_char, fmt: *const c::c_char, ap: VaList) -> c::c_int;
}

pub struct OwnedCStr {
    val: &'static CStr,
}

impl Deref for OwnedCStr {
    type Target = CStr;

    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl Drop for OwnedCStr {
    fn drop(&mut self) {
        unsafe {
            c::free(self.val.as_ptr() as _);
        }
    }
}

pub unsafe fn vasprintf_(fmt: *const c::c_char, ap: VaList) -> Option<OwnedCStr> {
    let mut res = ptr::null_mut();
    if vasprintf(&mut res, fmt, ap) == -1 {
        return None;
    }
    Some(OwnedCStr {
        val: CStr::from_ptr(res),
    })
}
