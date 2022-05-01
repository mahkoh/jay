use {
    crate::{
        it::test_error::TestError,
        utils::{oserror::OsError, ptr_ext::PtrExt},
    },
    std::{cell::Cell, ops::Deref, ptr, rc::Rc},
    uapi::{c, OwnedFd},
};

pub struct TestMem {
    pub fd: Rc<OwnedFd>,
    slice: *const [Cell<u8>],
}

impl TestMem {
    pub fn new(size: usize) -> Result<Rc<Self>, TestError> {
        let fd = uapi::memfd_create("test_pool", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING)?;
        uapi::fcntl_add_seals(fd.raw(), c::F_SEAL_SHRINK)?;
        uapi::ftruncate(fd.raw(), size as _)?;
        let slice = map(fd.raw(), size)?;
        Ok(Rc::new(Self {
            fd: Rc::new(fd),
            slice,
        }))
    }

    pub fn grow(&self, size: usize) -> Result<Rc<Self>, TestError> {
        let cur_len = uapi::fstat(self.fd.raw())?;
        if size > cur_len.st_size as _ {
            uapi::ftruncate(self.fd.raw(), size as _)?;
        }
        let slice = map(self.fd.raw(), size)?;
        Ok(Rc::new(Self {
            fd: self.fd.clone(),
            slice,
        }))
    }
}

impl Deref for TestMem {
    type Target = [Cell<u8>];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.slice }
    }
}

fn map(fd: c::c_int, size: usize) -> Result<*const [Cell<u8>], TestError> {
    if size == 0 {
        return Ok(&[]);
    }
    unsafe {
        let res = c::mmap(
            ptr::null_mut(),
            size as _,
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED,
            fd,
            0,
        );
        if res == c::MAP_FAILED {
            bail!("Could not map memory: {}", OsError::default());
        }
        Ok(std::slice::from_raw_parts(res as _, size))
    }
}

impl Drop for TestMem {
    fn drop(&mut self) {
        unsafe {
            c::munmap(self.slice.deref().as_ptr() as _, self.slice.deref().len());
        }
    }
}
