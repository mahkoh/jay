use {
    crate::{
        async_engine::AsyncEngine,
        dbus::{auth::handle_auth, DbusError, DbusHolder, DbusSocket},
        io_uring::IoUring,
        utils::{bufio::BufIo, errorfmt::ErrorFmt, numcell::NumCell, run_toplevel::RunToplevel},
        wire_dbus::org,
    },
    std::{cell::Cell, rc::Rc},
    uapi::c,
};

impl DbusHolder {
    pub(super) async fn get(
        self: &Rc<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        addr: &str,
        name: &'static str,
    ) -> Result<Rc<DbusSocket>, DbusError> {
        if let Some(c) = self.socket.get() {
            if c.dead.get() {
                self.socket.take();
            } else {
                return Ok(c);
            }
        }
        let socket = connect(eng, ring, addr, name, &self.run_toplevel).await?;
        self.socket.set(Some(socket.clone()));
        Ok(socket)
    }
}

async fn connect(
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    addr: &str,
    name: &'static str,
    run_toplevel: &Rc<RunToplevel>,
) -> Result<Rc<DbusSocket>, DbusError> {
    let fd = match uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
        Ok(s) => Rc::new(s),
        Err(e) => return Err(DbusError::Socket(e.into())),
    };
    let mut sadr: c::sockaddr_un = uapi::pod_zeroed();
    sadr.sun_family = c::AF_UNIX as _;
    let sun_path = uapi::as_bytes_mut(&mut sadr.sun_path[..]);
    sun_path[..addr.len()].copy_from_slice(addr.as_bytes());
    if let Err(e) = ring.connect(&fd, &sadr).await {
        return Err(DbusError::Connect(e));
    }
    let socket = Rc::new(DbusSocket {
        bus_name: name,
        fd: fd.clone(),
        ring: ring.clone(),
        in_bufs: Default::default(),
        bufio: Rc::new(BufIo::new(&fd, ring)),
        eng: eng.clone(),
        next_serial: NumCell::new(1),
        unique_name: Default::default(),
        reply_handlers: Default::default(),
        incoming: Default::default(),
        outgoing_: Default::default(),
        auth: Default::default(),
        dead: Cell::new(false),
        headers: Default::default(),
        run_toplevel: run_toplevel.clone(),
        signal_handlers: Default::default(),
        objects: Default::default(),
    });
    let skt = socket.clone();
    socket.call(
        "org.freedesktop.DBus",
        "/org/freedesktop/dbus",
        org::freedesktop::dbus::Hello,
        move |res| match res {
            Ok(name) => {
                log::info!("{}: Acquired unique name {}", skt.bus_name, name.name);
                let _ = skt.unique_name.set(Rc::new(name.name.to_string()));
            }
            Err(e) => {
                log::error!("{}: Hello call failed: {}", skt.bus_name, ErrorFmt(e));
                skt.kill();
            }
        },
    );
    let future = eng.spawn(handle_auth(socket.clone()));
    socket.auth.set(Some(future));
    Ok(socket)
}
