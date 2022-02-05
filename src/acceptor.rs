use crate::client::ClientError;
use crate::event_loop::{EventLoopDispatcher, EventLoopError, EventLoopId};
use crate::state::State;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, Errno, OwnedFd};

#[derive(Debug, Error)]
pub enum AcceptorError {
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
    #[error("XDG_RUNTIME_DIR is too long to form a unix socket address")]
    XrdTooLong,
    #[error("Could not create a wayland socket")]
    SocketFailed(#[source] std::io::Error),
    #[error("Could not start listening for incoming connections")]
    ListenFailed(#[source] std::io::Error),
    #[error("The wayland socket is in an error state")]
    ErrorEvent,
    #[error("Could not accept new connections")]
    AcceptFailed(#[source] std::io::Error),
    #[error("Could not spawn an event handler for a new connection")]
    SpawnFailed(#[source] ClientError),
    #[error("Could not bind the socket to an address")]
    BindFailed(#[source] std::io::Error),
    #[error("All wayland addresses in the range 0..1000 are already in use")]
    AddressesInUse,
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
}

pub struct Acceptor {
    _unlinker: Unlinker,
    id: EventLoopId,
    fd: OwnedFd,
    global: Rc<State>,
}

struct Unlinker(String);

impl Drop for Unlinker {
    fn drop(&mut self) {
        let _ = uapi::unlink(self.0.as_str());
    }
}

fn socket_path(xrd: &str, id: u32) -> String {
    format!("{}/wayland-{}", xrd, id)
}

fn bind_socket(fd: i32, xdr: &str) -> Result<u32, AcceptorError> {
    let mut addr: c::sockaddr_un = uapi::pod_zeroed();
    addr.sun_family = c::AF_UNIX as _;
    for i in 0..1000 {
        let path = socket_path(xdr, i);
        if path.len() + 1 > addr.sun_path.len() {
            return Err(AcceptorError::XrdTooLong);
        }
        let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
        sun_path[..path.len()].copy_from_slice(path.as_bytes());
        sun_path[path.len()] = 0;
        match uapi::bind(fd, &addr) {
            Ok(()) => return Ok(i),
            Err(Errno(c::EADDRINUSE)) => {
                log::warn!("Socket {} is already in use", path);
            }
            Err(e) => return Err(AcceptorError::BindFailed(e.into())),
        }
    }
    Err(AcceptorError::AddressesInUse)
}

impl Acceptor {
    pub fn install(global: &Rc<State>) -> Result<(), AcceptorError> {
        let xrd = match std::env::var("XDG_RUNTIME_DIR") {
            Ok(d) => d,
            Err(_) => return Err(AcceptorError::XrdNotSet),
        };
        let fd = match uapi::socket(
            c::AF_UNIX,
            c::SOCK_STREAM | c::SOCK_NONBLOCK | c::SOCK_CLOEXEC,
            0,
        ) {
            Ok(f) => f,
            Err(e) => return Err(AcceptorError::SocketFailed(e.into())),
        };
        let socket_id = bind_socket(fd.raw(), &xrd)?;
        let socket_path = socket_path(&xrd, socket_id);
        log::info!("bound to socket {}", socket_path);
        let unlinker = Unlinker(socket_path);
        if let Err(e) = uapi::listen(fd.raw(), 4096) {
            return Err(AcceptorError::ListenFailed(e.into()));
        }
        let id = global.el.id();
        let acc = Rc::new(Acceptor {
            _unlinker: unlinker,
            id,
            fd,
            global: global.clone(),
        });
        global.el.insert(id, Some(acc.fd.raw()), c::EPOLLIN, acc)?;
        Ok(())
    }
}

impl EventLoopDispatcher for Acceptor {
    fn dispatch(self: Rc<Self>, events: i32) -> Result<(), Box<dyn std::error::Error>> {
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            return Err(Box::new(AcceptorError::ErrorEvent));
        }
        loop {
            let fd = match uapi::accept4(
                self.fd.raw(),
                uapi::sockaddr_none_mut(),
                c::SOCK_NONBLOCK | c::SOCK_CLOEXEC,
            ) {
                Ok((fd, _)) => fd,
                Err(Errno(c::EAGAIN)) => break,
                Err(e) => return Err(Box::new(AcceptorError::AcceptFailed(e.into()))),
            };
            let id = self.global.clients.id();
            if let Err(e) = self.global.clients.spawn(id, &self.global, fd) {
                return Err(Box::new(AcceptorError::SpawnFailed(e)));
            }
        }
        Ok(())
    }
}

impl Drop for Acceptor {
    fn drop(&mut self) {
        let _ = self.global.el.remove(self.id);
    }
}
