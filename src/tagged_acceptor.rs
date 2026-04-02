use {
    crate::{
        async_engine::SpawnedFuture,
        client::ClientCaps,
        security_context_acceptor::AcceptorMetadata,
        state::State,
        utils::{
            errorfmt::ErrorFmt,
            numcell::NumCell,
            oserror::{OsError, OsErrorExt, OsErrorExt2},
            xrd::xrd,
        },
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{OwnedFd, Ustring, c, format_ustr},
};

#[derive(Debug, Error)]
pub enum TaggedAcceptorError {
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
}

#[derive(Default)]
pub struct TaggedAcceptors {
    acceptors: RefCell<AHashMap<String, Rc<Acceptor>>>,
    next_name: NumCell<u64>,
}

struct Acceptor {
    socket: AllocatedSocket,
    tag: String,
    state: Rc<State>,
    metadata: Rc<AcceptorMetadata>,
    future: Cell<Option<SpawnedFuture<()>>>,
}

impl TaggedAcceptors {
    pub fn clear(&self) {
        let acceptors = self.acceptors.take();
        for (_, acceptor) in acceptors {
            acceptor.kill();
        }
    }

    pub fn get(&self, state: &Rc<State>, tag: &str) -> Result<Rc<String>, TaggedAcceptorError> {
        let acceptors = &mut *self.acceptors.borrow_mut();
        if let Some(acceptor) = acceptors.get(tag) {
            return Ok(acceptor.socket.name.clone());
        }
        let acceptor = Rc::new(Acceptor {
            socket: self.allocate_socket()?,
            tag: tag.to_owned(),
            state: state.clone(),
            metadata: Rc::new(AcceptorMetadata {
                secure: false,
                sandboxed: false,
                sandbox_engine: Default::default(),
                app_id: Default::default(),
                instance_id: Default::default(),
                tag: Some(tag.to_owned()),
            }),
            future: Default::default(),
        });
        log::info!("Creating tagged acceptor `{tag}`");
        acceptor.future.set(Some(
            state.eng.spawn("tagged accept", acceptor.clone().accept()),
        ));
        acceptors.insert(tag.to_owned(), acceptor.clone());
        Ok(acceptor.socket.name.clone())
    }

    fn allocate_socket(&self) -> Result<AllocatedSocket, TaggedAcceptorError> {
        let xrd = xrd().ok_or(TaggedAcceptorError::XrdNotSet)?;
        let socket = uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0)
            .map(Rc::new)
            .map_os_err(TaggedAcceptorError::SocketFailed)?;
        loop {
            let i = self.next_name.fetch_add(1) + 1000;
            if let Some(s) = bind_socket(&socket, &xrd, i)? {
                return Ok(s);
            }
        }
    }
}

impl Acceptor {
    fn kill(&self) {
        log::info!("Destroying tagged acceptor `{}`", self.tag);
        self.future.take();
        self.state
            .tagged_acceptors
            .acceptors
            .borrow_mut()
            .remove(&self.tag);
    }

    async fn accept(self: Rc<Self>) {
        let s = &self.state;
        loop {
            let fd = match s.ring.accept(&self.socket.socket, c::SOCK_CLOEXEC).await {
                Ok(fd) => fd,
                Err(e) => {
                    log::error!("Could not accept a client: {}", ErrorFmt(e));
                    break;
                }
            };
            let id = s.clients.id();
            if let Err(e) = s
                .clients
                .spawn(id, s, fd, ClientCaps::all(), false, &self.metadata)
            {
                log::error!("Could not spawn a client: {}", ErrorFmt(e));
                break;
            }
        }
        self.kill();
    }
}

struct AllocatedSocket {
    // wayland-x
    name: Rc<String>,
    // /run/user/1000/wayland-x
    path: Ustring,
    socket: Rc<OwnedFd>,
    // /run/user/1000/wayland-x.lock
    lock_path: Ustring,
    _lock_fd: OwnedFd,
}

impl Drop for AllocatedSocket {
    fn drop(&mut self) {
        let _ = uapi::unlink(&self.path);
        let _ = uapi::unlink(&self.lock_path);
    }
}

fn bind_socket(
    fd: &Rc<OwnedFd>,
    xrd: &str,
    id: u64,
) -> Result<Option<AllocatedSocket>, TaggedAcceptorError> {
    let mut addr: c::sockaddr_un = uapi::pod_zeroed();
    addr.sun_family = c::AF_UNIX as _;
    let name = Rc::new(format!("wayland-{}", id));
    let path = format_ustr!("{}/{}", xrd, name);
    let lock_path = format_ustr!("{}.lock", path.display());
    if path.len() + 1 > addr.sun_path.len() {
        return Err(TaggedAcceptorError::XrdTooLong(xrd.to_string()));
    }
    let lock_fd = uapi::open(&*lock_path, c::O_CREAT | c::O_CLOEXEC | c::O_RDWR, 0o644)
        .map_os_err(TaggedAcceptorError::OpenLockFile)?;
    if let Err(e) = uapi::flock(lock_fd.raw(), c::LOCK_EX | c::LOCK_NB).to_os_error() {
        if e.0 == c::EWOULDBLOCK {
            return Ok(None);
        }
        return Err(TaggedAcceptorError::LockLockFile(e));
    }
    match uapi::lstat(&path).to_os_error() {
        Ok(_) => {
            log::info!("Unlinking {}", path.display());
            let _ = uapi::unlink(&path);
        }
        Err(OsError(c::ENOENT)) => {}
        Err(e) => return Err(TaggedAcceptorError::SocketStat(e)),
    }
    let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
    sun_path[..path.len()].copy_from_slice(path.as_bytes());
    sun_path[path.len()] = 0;
    uapi::bind(fd.raw(), &addr).map_os_err(TaggedAcceptorError::BindFailed)?;
    uapi::listen(fd.raw(), 4096).map_os_err(TaggedAcceptorError::ListenFailed)?;
    Ok(Some(AllocatedSocket {
        name,
        path,
        socket: fd.clone(),
        lock_path,
        _lock_fd: lock_fd,
    }))
}
