#![allow(dead_code)]

use std::cell::Cell;
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use thiserror::Error;
use uapi::{c, Errno, OwnedFd};

#[derive(Debug, Error)]
pub enum ServerMemError {
    #[error("memfd_create failed")]
    MemfdCreate(#[source] crate::utils::oserror::OsError),
    #[error("The provided size does not fit into off_t")]
    SizeOverflow,
    #[error("ftruncate failed")]
    Ftruncate(#[source] crate::utils::oserror::OsError),
    #[error("mmap failed")]
    MmapFailed(#[source] crate::utils::oserror::OsError),
    #[error("sealing failed")]
    Seal(#[source] crate::utils::oserror::OsError),
}

pub struct ServerMem {
    fd: OwnedFd,
    mem: *const [Cell<u8>],
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

impl ServerMem {
    pub fn new(size: usize) -> Result<Self, ServerMemError> {
        let name = format!("servermem-{}", NEXT_ID.fetch_add(1, Relaxed));
        let fd = match uapi::memfd_create(name, c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING) {
            Ok(f) => f,
            Err(e) => return Err(ServerMemError::MemfdCreate(e.into())),
        };
        let o_size = match size.try_into() {
            Ok(s) => s,
            _ => return Err(ServerMemError::SizeOverflow),
        };
        if let Err(e) = uapi::ftruncate(fd.raw(), o_size) {
            return Err(ServerMemError::Ftruncate(e.into()));
        }
        if let Err(e) =
            uapi::fcntl_add_seals(fd.raw(), c::F_SEAL_SHRINK | c::F_SEAL_GROW | c::F_SEAL_SEAL)
        {
            return Err(ServerMemError::Seal(e.into()));
        }
        let mem = unsafe {
            let res = c::mmap64(
                ptr::null_mut(),
                size,
                c::PROT_READ | c::PROT_WRITE,
                c::MAP_SHARED,
                fd.raw(),
                0,
            );
            if res == c::MAP_FAILED {
                return Err(ServerMemError::MmapFailed(Errno::default().into()));
            }
            std::slice::from_raw_parts(res as *mut Cell<u8>, size)
        };
        Ok(Self { fd, mem })
    }

    pub fn access<T, F: FnOnce(&[Cell<u8>]) -> T>(&self, f: F) -> T {
        unsafe { f(&*self.mem) }
    }

    pub fn fd(&self) -> i32 {
        self.fd.raw()
    }
}

impl Drop for ServerMem {
    fn drop(&mut self) {
        unsafe {
            c::munmap(self.mem as *const _ as _, (*self.mem).len());
        }
    }
}
