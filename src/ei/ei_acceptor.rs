use {
    crate::{
        async_engine::SpawnedFuture,
        state::State,
        utils::{errorfmt::ErrorFmt, oserror::OsError, xrd::xrd},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::{c, format_ustr, Errno, OwnedFd, Ustring},
};

#[derive(Debug, Error)]
pub enum EiAcceptorError {
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
    #[error("XDG_RUNTIME_DIR ({0:?}) is too long to form a unix socket address")]
    XrdTooLong(String),
    #[error("Could not create a libei socket")]
    SocketFailed(#[source] OsError),
    #[error("Could not stat the existing socket")]
    SocketStat(#[source] OsError),
    #[error("Could not start listening for incoming connections")]
    ListenFailed(#[source] OsError),
    #[error("Could not open the lock file")]
    OpenLockFile(#[source] OsError),
    #[error("Could not lock the lock file")]
    LockLockFile(#[source] OsError),
    #[error("Could not bind the socket to an address")]
    BindFailed(#[source] OsError),
    #[error("All libei addresses in the range 0..1000 are already in use")]
    AddressesInUse,
}

pub struct EiAcceptor {
    socket: EiAllocatedSocket,
}

struct EiAllocatedSocket {
    // eis-x
    name: String,
    // /run/user/1000/eis-x
    path: Ustring,
    insecure: Rc<OwnedFd>,
    // /run/user/1000/eis-x.lock
    lock_path: Ustring,
    _lock_fd: OwnedFd,
}

impl Drop for EiAllocatedSocket {
    fn drop(&mut self) {
        let _ = uapi::unlink(&self.path);
        let _ = uapi::unlink(&self.lock_path);
    }
}

fn bind_socket(
    insecure: &Rc<OwnedFd>,
    xrd: &str,
    id: u32,
) -> Result<EiAllocatedSocket, EiAcceptorError> {
    let mut addr: c::sockaddr_un = uapi::pod_zeroed();
    addr.sun_family = c::AF_UNIX as _;
    let name = format!("eis-{}", id);
    let path = format_ustr!("{}/{}", xrd, name);
    let lock_path = format_ustr!("{}.lock", path.display());
    if path.len() + 1 > addr.sun_path.len() {
        return Err(EiAcceptorError::XrdTooLong(xrd.to_string()));
    }
    let lock_fd = match uapi::open(&*lock_path, c::O_CREAT | c::O_CLOEXEC | c::O_RDWR, 0o644) {
        Ok(l) => l,
        Err(e) => return Err(EiAcceptorError::OpenLockFile(e.into())),
    };
    if let Err(e) = uapi::flock(lock_fd.raw(), c::LOCK_EX | c::LOCK_NB) {
        return Err(EiAcceptorError::LockLockFile(e.into()));
    }
    match uapi::lstat(&path) {
        Ok(_) => {
            log::info!("Unlinking {}", path.display());
            let _ = uapi::unlink(&path);
        }
        Err(Errno(c::ENOENT)) => {}
        Err(e) => return Err(EiAcceptorError::SocketStat(e.into())),
    }
    let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
    sun_path[..path.len()].copy_from_slice(path.as_bytes());
    sun_path[path.len()] = 0;
    if let Err(e) = uapi::bind(insecure.raw(), &addr) {
        return Err(EiAcceptorError::BindFailed(e.into()));
    }
    Ok(EiAllocatedSocket {
        name,
        path,
        insecure: insecure.clone(),
        lock_path,
        _lock_fd: lock_fd,
    })
}

fn allocate_socket() -> Result<EiAllocatedSocket, EiAcceptorError> {
    let xrd = match xrd() {
        Some(d) => d,
        _ => return Err(EiAcceptorError::XrdNotSet),
    };
    let socket = match uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
        Ok(f) => Rc::new(f),
        Err(e) => return Err(EiAcceptorError::SocketFailed(e.into())),
    };
    for i in 1..1000 {
        match bind_socket(&socket, &xrd, i) {
            Ok(s) => return Ok(s),
            Err(e) => {
                log::warn!("Cannot use the eis-{} socket: {}", i, ErrorFmt(e));
            }
        }
    }
    Err(EiAcceptorError::AddressesInUse)
}

impl EiAcceptor {
    pub fn spawn(
        state: &Rc<State>,
    ) -> Result<(Rc<EiAcceptor>, SpawnedFuture<()>), EiAcceptorError> {
        let socket = allocate_socket()?;
        log::info!("bound to socket {}", socket.path.display());
        if let Err(e) = uapi::listen(socket.insecure.raw(), 4096) {
            return Err(EiAcceptorError::ListenFailed(e.into()));
        }
        let acc = Rc::new(EiAcceptor { socket });
        let future = state
            .eng
            .spawn(accept(acc.socket.insecure.clone(), state.clone()));
        Ok((acc, future))
    }

    pub fn socket_name(&self) -> &str {
        &self.socket.name
    }
}

async fn accept(fd: Rc<OwnedFd>, state: Rc<State>) {
    loop {
        let fd = match state.ring.accept(&fd, c::SOCK_CLOEXEC).await {
            Ok(fd) => fd,
            Err(e) => {
                log::error!("Could not accept a client: {}", ErrorFmt(e));
                break;
            }
        };
        if let Err(e) = state.ei_clients.spawn(&state, fd) {
            log::error!("Could not spawn a client: {}", ErrorFmt(e));
            break;
        }
    }
    state.ring.stop();
}
