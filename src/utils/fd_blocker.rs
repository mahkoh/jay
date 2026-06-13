use {
    crate::utils::{
        oserror::{OsError, OsErrorExt},
        pipe::pipe,
    },
    std::{rc::Rc, slice},
    uapi::{OwnedFd, c},
};

pub struct FdBlocker(#[expect(dead_code)] pub Rc<OwnedFd>);

pub struct FdBarrier(pub Rc<OwnedFd>);

pub fn create_fd_blocker() -> Result<(FdBlocker, FdBarrier), OsError> {
    let pipe = pipe()?;
    let blocker = FdBlocker(Rc::new(pipe.read));
    let barrier = FdBarrier(Rc::new(pipe.write));
    Ok((blocker, barrier))
}

impl FdBarrier {
    pub fn wait_blocking(&self) -> Result<(), OsError> {
        loop {
            let mut pollfd = c::pollfd {
                fd: self.0.raw(),
                events: 0,
                revents: 0,
            };
            let res = uapi::poll(slice::from_mut(&mut pollfd), -1).to_os_error();
            if let Err(e) = res {
                if e.0 == c::EINTR {
                    continue;
                }
                return Err(e);
            }
            if pollfd.revents & (c::POLLHUP | c::POLLERR) != 0 {
                return Ok(());
            }
        }
    }
}
