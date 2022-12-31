use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::IoUring,
        utils::{buf::TypedBuf, errorfmt::ErrorFmt, oserror::OsError},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum SighandError {
    #[error("Could not block the signalfd signals")]
    BlockFailed(#[source] OsError),
    #[error("Could not create a signalfd")]
    CreateFailed(#[source] OsError),
}

pub fn install(
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
    let fd = match uapi::signalfd_new(&set, c::SFD_CLOEXEC) {
        Ok(fd) => Rc::new(fd),
        Err(e) => return Err(SighandError::CreateFailed(e.into())),
    };
    Ok(eng.spawn(handle_signals(fd, ring.clone())))
}

async fn handle_signals(fd: Rc<OwnedFd>, ring: Rc<IoUring>) {
    let mut buf = TypedBuf::<c::signalfd_siginfo>::new();
    loop {
        if let Err(e) = ring.read(&fd, buf.buf()).await {
            log::error!("Could not read from signal fd: {}", ErrorFmt(e));
            return;
        }
        let sig = buf.t().ssi_signo as i32;
        log::info!("Received signal {}", sig);
        if matches!(sig, c::SIGINT | c::SIGTERM) {
            log::info!("Exiting");
            ring.stop();
        }
    }
}
