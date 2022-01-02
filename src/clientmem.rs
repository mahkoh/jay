use std::cell::{Cell, UnsafeCell};
use std::mem::MaybeUninit;
use std::sync::atomic::{compiler_fence, Ordering};
use std::{mem, ptr};
use thiserror::Error;
use uapi::c;
use uapi::c::raise;

#[derive(Debug, Error)]
pub enum ClientMemError {
    #[error("Could not install the sigbus handler")]
    SigactionFailed(#[source] std::io::Error),
    #[error("A SIGBUS occurred while accessing mapped memory")]
    Sigbus,
    #[error("mmap failed")]
    MmapFailed(#[source] std::io::Error),
}

pub struct ClientMem {
    failed: Cell<bool>,
    sigbus_impossible: bool,
    data: *mut [Cell<u8>],
}

impl ClientMem {
    pub fn new(fd: i32, len: usize) -> Result<Self, ClientMemError> {
        let mut sigbus_impossible = false;
        if let Ok(seals) = uapi::fcntl_get_seals(fd) {
            if seals & c::F_SEAL_SHRINK != 0 {
                if let Ok(stat) = uapi::fstat(fd) {
                    sigbus_impossible = stat.st_size as u64 >= len as u64;
                }
            }
        }
        let data = unsafe {
            let data = c::mmap64(
                ptr::null_mut(),
                len,
                c::PROT_READ | c::PROT_WRITE,
                c::MAP_SHARED,
                fd,
                0,
            );
            if data == c::MAP_FAILED {
                return Err(ClientMemError::MmapFailed(uapi::Errno::default().into()));
            }
            std::slice::from_raw_parts_mut(data as *mut Cell<u8>, len)
        };
        Ok(Self {
            failed: Cell::new(false),
            sigbus_impossible,
            data,
        })
    }

    pub fn access<T, F: FnOnce(&[Cell<u8>]) -> T>(&self, f: F) -> Result<T, ClientMemError> {
        unsafe {
            if self.sigbus_impossible {
                return Ok(f(&mut *self.data));
            }
            MEM.with(|m| {
                let mref = MemRef {
                    mem: self,
                    outer: *m.get(),
                };
                *m.get() = &mref;
                compiler_fence(Ordering::SeqCst);
                let res = f(&mut *self.data);
                *m.get() = mref.outer;
                compiler_fence(Ordering::SeqCst);
                match self.failed.get() {
                    true => Err(ClientMemError::Sigbus),
                    _ => Ok(res),
                }
            })
        }
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.data).len() }
    }
}

impl Drop for ClientMem {
    fn drop(&mut self) {
        unsafe {
            c::munmap(self.data as _, self.len());
        }
    }
}

struct MemRef {
    mem: *const ClientMem,
    outer: *const MemRef,
}

thread_local! {
    static MEM: UnsafeCell<*const MemRef> = UnsafeCell::new(ptr::null());
}

unsafe fn kill() -> ! {
    c::signal(c::SIGBUS, c::SIG_DFL);
    raise(c::SIGBUS);
    unreachable!();
}

unsafe extern "C" fn sigbus(sig: i32, info: &c::siginfo_t, _ucontext: *mut c::c_void) {
    assert_eq!(sig, c::SIGBUS);
    let mut memr_ptr = MEM.with(|m| ptr::read(m.get()));
    while !memr_ptr.is_null() {
        let memr = &*memr_ptr;
        let mem = &*memr.mem;
        let lo = mem.data as *mut u8 as usize;
        let hi = lo + mem.len();
        let fault_addr = info.si_addr() as usize;
        if fault_addr < lo || fault_addr >= hi {
            memr_ptr = memr.outer;
            continue;
        }
        let res = c::mmap64(
            lo as _,
            hi - lo,
            c::PROT_WRITE | c::PROT_READ,
            c::MAP_ANONYMOUS | c::MAP_PRIVATE,
            -1,
            0,
        );
        if res == c::MAP_FAILED {
            kill();
        }
        mem.failed.set(true);
        return;
    }
    kill();
}

pub fn init() -> Result<(), ClientMemError> {
    unsafe {
        let mut action: c::sigaction = MaybeUninit::zeroed().assume_init();
        action.sa_sigaction =
            mem::transmute(sigbus as unsafe extern "C" fn(i32, &c::siginfo_t, *mut c::c_void));
        action.sa_flags = c::SA_NODEFER | c::SA_SIGINFO;
        let res = c::sigaction(c::SIGBUS, &action, ptr::null_mut());
        match uapi::map_err!(res) {
            Ok(_) => Ok(()),
            Err(e) => Err(ClientMemError::SigactionFailed(e.into())),
        }
    }
}
