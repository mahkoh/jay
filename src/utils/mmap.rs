use {
    crate::utils::{oserror::OsError, ptr_ext::PtrExt},
    std::ptr,
    uapi::c,
};

pub struct Mmapped {
    pub ptr: *const [u8],
}

pub fn mmap(
    len: usize,
    prot: c::c_int,
    flags: c::c_int,
    fd: c::c_int,
    offset: c::off_t,
) -> Result<Mmapped, OsError> {
    let res = unsafe { c::mmap(ptr::null_mut(), len, prot, flags, fd, offset) };
    if res == c::MAP_FAILED {
        Err(OsError::default())
    } else {
        Ok(Mmapped {
            ptr: unsafe { std::slice::from_raw_parts(res.cast(), len) },
        })
    }
}

impl Drop for Mmapped {
    fn drop(&mut self) {
        unsafe {
            c::munmap(self.ptr.deref().as_ptr() as _, self.ptr.deref().len());
        }
    }
}
