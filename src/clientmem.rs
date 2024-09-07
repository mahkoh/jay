use {
    crate::{client::Client, utils::vec_ext::VecExt},
    std::{
        cell::Cell,
        mem::MaybeUninit,
        ptr,
        rc::Rc,
        sync::atomic::{compiler_fence, Ordering},
    },
    thiserror::Error,
    uapi::{
        c::{self, raise},
        OwnedFd,
    },
};

#[derive(Debug, Error)]
pub enum ClientMemError {
    #[error("Could not install the sigbus handler")]
    SigactionFailed(#[source] crate::utils::oserror::OsError),
    #[error("A SIGBUS occurred while accessing mapped memory")]
    Sigbus,
    #[error("mmap failed")]
    MmapFailed(#[source] crate::utils::oserror::OsError),
}

pub struct ClientMem {
    fd: Rc<OwnedFd>,
    failed: Cell<bool>,
    sigbus_impossible: bool,
    data: *const [Cell<u8>],
}

#[derive(Clone)]
pub struct ClientMemOffset {
    mem: Rc<ClientMem>,
    offset: usize,
    data: *const [Cell<u8>],
}

impl ClientMem {
    pub fn new(
        fd: &Rc<OwnedFd>,
        len: usize,
        read_only: bool,
        client: Option<&Client>,
    ) -> Result<Self, ClientMemError> {
        let mut sigbus_impossible = false;
        if let Ok(seals) = uapi::fcntl_get_seals(fd.raw()) {
            if seals & c::F_SEAL_SHRINK != 0 {
                if let Ok(stat) = uapi::fstat(fd.raw()) {
                    sigbus_impossible = stat.st_size as u64 >= len as u64;
                }
            }
        }
        if !sigbus_impossible {
            if let Some(client) = client {
                log::debug!(
                    "Client {} ({}) has created a shm buffer that might cause SIGBUS",
                    client.pid_info.comm,
                    client.id,
                );
            }
        }
        let data = if len == 0 {
            &mut [][..]
        } else {
            let prot = match read_only {
                true => c::PROT_READ,
                false => c::PROT_READ | c::PROT_WRITE,
            };
            unsafe {
                let data = c::mmap64(ptr::null_mut(), len, prot, c::MAP_SHARED, fd.raw(), 0);
                if data == c::MAP_FAILED {
                    return Err(ClientMemError::MmapFailed(uapi::Errno::default().into()));
                }
                std::slice::from_raw_parts_mut(data as *mut Cell<u8>, len)
            }
        };
        Ok(Self {
            fd: fd.clone(),
            failed: Cell::new(false),
            sigbus_impossible,
            data,
        })
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.data).len() }
    }

    pub fn offset(self: &Rc<Self>, offset: usize) -> ClientMemOffset {
        let mem = unsafe { &*self.data };
        ClientMemOffset {
            mem: self.clone(),
            offset,
            data: &mem[offset..],
        }
    }

    pub fn fd(&self) -> &Rc<OwnedFd> {
        &self.fd
    }

    pub fn sigbus_impossible(&self) -> bool {
        self.sigbus_impossible
    }
}

impl ClientMemOffset {
    pub fn pool(&self) -> &ClientMem {
        &self.mem
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn ptr(&self) -> *const [Cell<u8>] {
        self.data
    }

    pub fn access<T, F: FnOnce(&[Cell<u8>]) -> T>(&self, f: F) -> Result<T, ClientMemError> {
        unsafe {
            if self.mem.sigbus_impossible {
                return Ok(f(&*self.data));
            }
            let mref = MemRef {
                mem: &*self.mem,
                outer: MEM.get(),
            };
            MEM.set(&mref);
            compiler_fence(Ordering::SeqCst);
            let res = f(&*self.data);
            MEM.set(mref.outer);
            compiler_fence(Ordering::SeqCst);
            match self.mem.failed.get() {
                true => Err(ClientMemError::Sigbus),
                _ => Ok(res),
            }
        }
    }

    pub fn read(&self, dst: &mut Vec<u8>) -> Result<(), ClientMemError> {
        self.access(|v| {
            dst.reserve(v.len());
            let (_, unused) = dst.split_at_spare_mut_ext();
            unused[..v.len()].copy_from_slice(uapi::as_maybe_uninit_bytes(v));
            unsafe {
                dst.set_len(dst.len() + v.len());
            }
        })
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
    static MEM: Cell<*const MemRef> = const { Cell::new(ptr::null()) };
}

unsafe fn kill() -> ! {
    c::signal(c::SIGBUS, c::SIG_DFL);
    raise(c::SIGBUS);
    unreachable!();
}

unsafe extern "C" fn sigbus(sig: i32, info: &c::siginfo_t, _ucontext: *mut c::c_void) {
    assert_eq!(sig, c::SIGBUS);
    let mut memr_ptr = MEM.get();
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
            c::MAP_ANONYMOUS | c::MAP_PRIVATE | c::MAP_FIXED,
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
            sigbus as unsafe extern "C" fn(i32, &c::siginfo_t, *mut c::c_void) as _;
        action.sa_flags = c::SA_NODEFER | c::SA_SIGINFO;
        let res = c::sigaction(c::SIGBUS, &action, ptr::null_mut());
        match uapi::map_err!(res) {
            Ok(_) => Ok(()),
            Err(e) => Err(ClientMemError::SigactionFailed(e.into())),
        }
    }
}
