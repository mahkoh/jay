use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        event_loop::{EventLoop, EventLoopError},
        io_uring::IoUring,
        utils::{errorfmt::ErrorFmt, oserror::OsError},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::{c, Errno, OwnedFd},
};

#[derive(Debug, Error)]
pub enum SighandError {
    #[error("Could not block the signalfd signals")]
    BlockFailed(#[source] OsError),
    #[error("Could not create a signalfd")]
    CreateFailed(#[source] OsError),
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
}

pub fn install(
    el: &Rc<EventLoop>,
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
) -> Result<SpawnedFuture<()>, SighandError> {
    let mut set: c::sigset_t = uapi::pod_zeroed();
    uapi::sigaddset(&mut set, c::SIGINT).unwrap();
    uapi::sigaddset(&mut set, c::SIGTERM).unwrap();
    uapi::sigaddset(&mut set, c::SIGPIPE).unwrap();
    if let Err(e) = uapi::pthread_sigmask(c::SIG_BLOCK, Some(&set), None) {
        return Err(SighandError::BlockFailed(e.into()));
    }
    let fd = match uapi::signalfd_new(&set, c::SFD_CLOEXEC | c::SFD_NONBLOCK) {
        Ok(fd) => Rc::new(fd),
        Err(e) => return Err(SighandError::CreateFailed(e.into())),
    };
    Ok(eng.spawn(handle_signals(fd, ring.clone(), el.clone())))
}

async fn handle_signals(fd: Rc<OwnedFd>, ring: Rc<IoUring>, el: Rc<EventLoop>) {
    let mut siginfo: c::signalfd_siginfo = uapi::pod_zeroed();
    loop {
        if let Err(e) = ring.readable(&fd).await {
            log::error!(
                "Could not wait for signal fd to become readable: {}",
                ErrorFmt(e)
            );
            return;
        }
        loop {
            if let Err(e) = uapi::read(fd.raw(), &mut siginfo) {
                match e {
                    Errno(c::EAGAIN) => break,
                    _ => {
                        log::error!(
                            "Could not read from signal fd: {}",
                            ErrorFmt(OsError::from(e))
                        );
                        return;
                    }
                }
            }
            let sig = siginfo.ssi_signo as i32;
            log::info!("Received signal {}", sig);
            if matches!(sig, c::SIGINT | c::SIGTERM) {
                log::info!("Exiting");
                el.stop();
            }
        }
    }
}
