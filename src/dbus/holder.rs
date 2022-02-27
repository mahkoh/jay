use crate::dbus::auth::handle_auth;
use crate::dbus::{DbusError, DbusHolder, DbusSocket};
use crate::{AsyncEngine, NumCell};
use std::cell::Cell;
use std::rc::Rc;
use uapi::c;

impl DbusHolder {
    pub(super) fn get(
        self: &Rc<Self>,
        eng: &Rc<AsyncEngine>,
        addr: &str,
    ) -> Result<Rc<DbusSocket>, DbusError> {
        if let Some(c) = self.socket.get() {
            if c.dead.get() {
                self.socket.take();
            } else {
                return Ok(c);
            }
        }
        let socket = connect(eng, addr)?;
        self.socket.set(Some(socket.clone()));
        Ok(socket)
    }
}

fn connect(eng: &Rc<AsyncEngine>, addr: &str) -> Result<Rc<DbusSocket>, DbusError> {
    let socket = match uapi::socket(
        c::AF_UNIX,
        c::SOCK_STREAM | c::SOCK_NONBLOCK | c::SOCK_CLOEXEC,
        0,
    ) {
        Ok(s) => s,
        Err(e) => return Err(DbusError::Socket(e.into())),
    };
    let mut sadr: c::sockaddr_un = uapi::pod_zeroed();
    sadr.sun_family = c::AF_UNIX as _;
    let sun_path = uapi::as_bytes_mut(&mut sadr.sun_path[..]);
    sun_path[..addr.len()].copy_from_slice(addr.as_bytes());
    if let Err(e) = uapi::connect(socket.raw(), &sadr) {
        return Err(DbusError::Connect(e.into()));
    }
    let socket = Rc::new(DbusSocket {
        fd: eng.fd(&Rc::new(socket))?,
        eng: eng.clone(),
        next_serial: NumCell::new(1),
        bufs: Default::default(),
        outgoing: Default::default(),
        waiters: Default::default(),
        replies: Default::default(),
        incoming: Default::default(),
        outgoing_: Default::default(),
        auth: Default::default(),
        dead: Cell::new(false),
        headers: Default::default(),
    });
    let future = eng.spawn(handle_auth(socket.clone()));
    socket.auth.set(Some(future));
    Ok(socket)
}
