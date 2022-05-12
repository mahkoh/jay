use {
    crate::{
        async_engine::SpawnedFuture,
        state::State,
        utils::{errorfmt::ErrorFmt, oserror::OsError, xrd::xrd},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::{c, format_ustr, Errno, OwnedFd, Ustr, Ustring},
};

#[derive(Debug, Error)]
pub enum AcceptorError {
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
    #[error("XDG_RUNTIME_DIR ({0:?}) is too long to form a unix socket address")]
    XrdTooLong(String),
    #[error("Could not create a wayland socket")]
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
    #[error("All wayland addresses in the range 0..1000 are already in use")]
    AddressesInUse,
}

pub struct Acceptor {
    socket: AllocatedSocket,
}

struct AllocatedSocket {
    // wayland-x
    name: String,
    // /run/user/1000/wayland-x
    path: Ustring,
    insecure: Rc<OwnedFd>,
    // /run/user/1000/wayland-x.lock
    lock_path: Ustring,
    _lock_fd: OwnedFd,
    // /run/user/1000/wayland-x.jay
    #[cfg_attr(not(feature = "it"), allow(dead_code))]
    secure_path: Ustring,
    secure: Rc<OwnedFd>,
}

impl Drop for AllocatedSocket {
    fn drop(&mut self) {
        let _ = uapi::unlink(&self.path);
        let _ = uapi::unlink(&self.lock_path);
    }
}

fn bind_socket(
    insecure: &Rc<OwnedFd>,
    secure: &Rc<OwnedFd>,
    xrd: &str,
    id: u32,
) -> Result<AllocatedSocket, AcceptorError> {
    let mut addr: c::sockaddr_un = uapi::pod_zeroed();
    addr.sun_family = c::AF_UNIX as _;
    let name = format!("wayland-{}", id);
    let path = format_ustr!("{}/{}", xrd, name);
    let jay_path = format_ustr!("{}.jay", path.display());
    let lock_path = format_ustr!("{}.lock", path.display());
    if jay_path.len() + 1 > addr.sun_path.len() {
        return Err(AcceptorError::XrdTooLong(xrd.to_string()));
    }
    let lock_fd = match uapi::open(&*lock_path, c::O_CREAT | c::O_CLOEXEC | c::O_RDWR, 0o644) {
        Ok(l) => l,
        Err(e) => return Err(AcceptorError::OpenLockFile(e.into())),
    };
    if let Err(e) = uapi::flock(lock_fd.raw(), c::LOCK_EX | c::LOCK_NB) {
        return Err(AcceptorError::LockLockFile(e.into()));
    }
    for (name, fd) in [(&path, insecure), (&jay_path, secure)] {
        match uapi::lstat(name) {
            Ok(_) => {
                log::info!("Unlinking {}", name.display());
                let _ = uapi::unlink(name);
            }
            Err(Errno(c::ENOENT)) => {}
            Err(e) => return Err(AcceptorError::SocketStat(e.into())),
        }
        let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
        sun_path[..name.len()].copy_from_slice(name.as_bytes());
        sun_path[name.len()] = 0;
        if let Err(e) = uapi::bind(fd.raw(), &addr) {
            return Err(AcceptorError::BindFailed(e.into()));
        }
    }
    Ok(AllocatedSocket {
        name,
        path,
        insecure: insecure.clone(),
        lock_path,
        _lock_fd: lock_fd,
        secure_path: jay_path,
        secure: secure.clone(),
    })
}

fn allocate_socket() -> Result<AllocatedSocket, AcceptorError> {
    let xrd = match xrd() {
        Some(d) => d,
        _ => return Err(AcceptorError::XrdNotSet),
    };
    let mut fds = [None, None];
    for fd in &mut fds {
        let socket = match uapi::socket(
            c::AF_UNIX,
            c::SOCK_STREAM | c::SOCK_NONBLOCK | c::SOCK_CLOEXEC,
            0,
        ) {
            Ok(f) => Rc::new(f),
            Err(e) => return Err(AcceptorError::SocketFailed(e.into())),
        };
        *fd = Some(socket);
    }
    let unsecure = fds[0].take().unwrap();
    let secure = fds[1].take().unwrap();
    for i in 1..1000 {
        match bind_socket(&unsecure, &secure, &xrd, i) {
            Ok(s) => return Ok(s),
            Err(e) => {
                log::warn!("Cannot use the wayland-{} socket: {}", i, ErrorFmt(e));
            }
        }
    }
    Err(AcceptorError::AddressesInUse)
}

impl Acceptor {
    pub fn install(
        state: &Rc<State>,
    ) -> Result<(Rc<Acceptor>, Vec<SpawnedFuture<()>>), AcceptorError> {
        let socket = allocate_socket()?;
        log::info!("bound to socket {}", socket.path.display());
        for fd in [&socket.secure, &socket.insecure] {
            if let Err(e) = uapi::listen(fd.raw(), 4096) {
                return Err(AcceptorError::ListenFailed(e.into()));
            }
        }
        let acc = Rc::new(Acceptor { socket });
        let mut futures = vec![];
        futures.push(
            state
                .eng
                .spawn(accept(acc.socket.secure.clone(), state.clone(), true)),
        );
        futures.push(
            state
                .eng
                .spawn(accept(acc.socket.insecure.clone(), state.clone(), false)),
        );
        state.acceptor.set(Some(acc.clone()));
        Ok((acc, futures))
    }

    pub fn socket_name(&self) -> &str {
        &self.socket.name
    }

    #[cfg_attr(not(feature = "it"), allow(dead_code))]
    pub fn secure_path(&self) -> &Ustr {
        self.socket.secure_path.as_ustr()
    }
}

async fn accept(fd: Rc<OwnedFd>, state: Rc<State>, secure: bool) {
    loop {
        if let Err(e) = state.ring.readable(&fd).await {
            log::error!(
                "Could not wait for the acceptor to become readable: {}",
                ErrorFmt(e)
            );
            break;
        }
        loop {
            let fd = match uapi::accept4(
                fd.raw(),
                uapi::sockaddr_none_mut(),
                c::SOCK_NONBLOCK | c::SOCK_CLOEXEC,
            ) {
                Ok((fd, _)) => fd,
                Err(Errno(c::EAGAIN)) => break,
                Err(e) => {
                    log::error!("Could not accept a client: {}", ErrorFmt(OsError::from(e)));
                    break;
                }
            };
            let id = state.clients.id();
            if let Err(e) = state.clients.spawn(id, &state, fd, secure) {
                log::error!("Could not spawn a client: {}", ErrorFmt(e));
                break;
            }
        }
    }
    state.ring.stop();
}
