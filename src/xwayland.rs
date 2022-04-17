mod xsocket;
mod xwm;

use {
    crate::{
        async_engine::AsyncError,
        client::ClientError,
        compositor::DISPLAY,
        forker::{ForkerError, ForkerProxy},
        ifs::wl_surface::{
            xwindow::{Xwindow, XwindowData},
            WlSurface,
        },
        state::State,
        user_session::import_environment,
        utils::{errorfmt::ErrorFmt, oserror::OsError, tri::Try},
        wire::WlSurfaceId,
        xcon::XconError,
        xwayland::{xsocket::allocate_socket, xwm::Wm},
    },
    bstr::ByteSlice,
    std::{num::ParseIntError, rc::Rc},
    thiserror::Error,
    uapi::{c, pipe2, Errno, OwnedFd},
};

#[derive(Debug, Error)]
enum XWaylandError {
    #[error("Could not create a wayland socket")]
    SocketFailed(#[source] OsError),
    #[error("/tmp/.X11-unix does not exist")]
    MissingSocketDir,
    #[error("Could not stat /tmp/.X11-unix")]
    StatSocketDir(#[source] OsError),
    #[error("/tmp/.X11-unix is not a directory")]
    NotASocketDir,
    #[error("/tmp/.X11-unix is writable")]
    SocketDirNotWritable,
    #[error("Could not write to the lock file")]
    WriteLockFile(#[source] OsError),
    #[error("Could not open the lock file for reading")]
    ReadLockFile(#[source] OsError),
    #[error("The lock file does not contain a PID")]
    NotALockFile(#[source] ParseIntError),
    #[error("The socket is already in use")]
    AlreadyInUse,
    #[error("Could not bind the socket to an address")]
    BindFailed(#[source] OsError),
    #[error("All X displays in the range 0..1000 are already in use")]
    AddressesInUse,
    #[error("The async engine returned an error")]
    AsyncError(#[from] AsyncError),
    #[error("pipe(2) failed")]
    Pipe(#[source] OsError),
    #[error("socketpair(2) failed")]
    Socketpair(#[source] OsError),
    #[error("Could not start Xwayland")]
    ExecFailed(#[source] ForkerError),
    #[error("Could not load the atoms")]
    LoadAtoms(#[source] XconError),
    #[error("Could not connect to Xwayland")]
    Connect(#[source] XconError),
    #[error("Could not create a window manager")]
    CreateWm(#[source] Box<Self>),
    #[error("Could not select the root events")]
    SelectRootEvents(#[source] XconError),
    #[error("Could not create the WM window")]
    CreateXWindow(#[source] XconError),
    #[error("Could not set the cursor of the root window")]
    SetCursor(#[source] XconError),
    #[error("composite_redirect_subwindows failed")]
    CompositeRedirectSubwindows(#[source] XconError),
    #[error("Could not spawn the Xwayland client")]
    SpawnClient(#[source] ClientError),
    #[error("An unspecified XconError occurred")]
    XconError(#[from] XconError),
}

pub async fn manage(state: Rc<State>) {
    loop {
        let forker = match state.forker.get() {
            Some(f) => f,
            None => {
                log::error!("There is no forker. Cannot start Xwayland.");
                return;
            }
        };
        let (xsocket, socket) = match allocate_socket() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not allocate a socket for Xwayland: {}", ErrorFmt(e));
                return;
            }
        };
        if let Err(e) = uapi::listen(socket.raw(), 4096) {
            log::error!("Could not listen on the Xwayland socket: {}", ErrorFmt(e));
            return;
        }
        let display = format!(":{}", xsocket.id);
        forker.setenv(DISPLAY.as_bytes(), display.as_bytes());
        log::info!("Allocated display :{} for Xwayland", xsocket.id);
        log::info!("Waiting for connection attempt");
        if state.backend.get().is_freestanding() {
            import_environment(&state, DISPLAY, &display);
        }
        let res = XWaylandError::tria(async {
            state.eng.fd(&socket)?.readable().await?;
            Ok(())
        })
        .await;
        if let Err(e) = res {
            log::error!("{}", ErrorFmt(e));
            return;
        }
        log::info!("Starting Xwayland");
        if let Err(e) = run(&state, &forker, socket).await {
            log::error!("Xwayland failed: {}", ErrorFmt(e));
        } else {
            log::warn!("Xwayland exited unexpectedly");
        }
        forker.unsetenv(DISPLAY.as_bytes());
    }
}

async fn run(
    state: &Rc<State>,
    forker: &Rc<ForkerProxy>,
    socket: Rc<OwnedFd>,
) -> Result<(), XWaylandError> {
    let (dfdread, dfdwrite) = match pipe2(c::O_CLOEXEC) {
        Ok(p) => p,
        Err(e) => return Err(XWaylandError::Pipe(e.into())),
    };
    let (stderr_read, stderr_write) = match pipe2(c::O_CLOEXEC) {
        Ok(p) => p,
        Err(e) => return Err(XWaylandError::Pipe(e.into())),
    };
    let wm = uapi::socketpair(
        c::AF_UNIX,
        c::SOCK_STREAM | c::SOCK_CLOEXEC | c::SOCK_NONBLOCK,
        0,
    );
    let (wm1, wm2) = match wm {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::Socketpair(e.into())),
    };
    let client = uapi::socketpair(
        c::AF_UNIX,
        c::SOCK_STREAM | c::SOCK_CLOEXEC | c::SOCK_NONBLOCK,
        0,
    );
    let (client1, client2) = match client {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::Socketpair(e.into())),
    };
    let stderr_read = state.eng.spawn(log_xwayland(state.clone(), stderr_read));
    let pidfd = forker
        .xwayland(
            Rc::new(stderr_write),
            Rc::new(dfdwrite),
            socket,
            Rc::new(wm2),
            Rc::new(client2),
        )
        .await;
    let pidfd = match pidfd {
        Ok(p) => p,
        Err(e) => return Err(XWaylandError::ExecFailed(e)),
    };
    let client_id = state.clients.id();
    let client = state
        .clients
        .spawn2(client_id, state, client1, 9999, 9999, true, true);
    let client = match client {
        Ok(c) => c,
        Err(e) => return Err(XWaylandError::SpawnClient(e)),
    };
    state.eng.fd(&Rc::new(dfdread))?.readable().await?;
    state.xwayland.queue.clear();
    let wm = match Wm::get(state, client, wm1).await {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::CreateWm(Box::new(e))),
    };
    let wm = state.eng.spawn(wm.run());
    state.eng.fd(&Rc::new(pidfd))?.readable().await?;
    drop(wm);
    state.xwayland.queue.clear();
    stderr_read.await;
    Ok(())
}

pub fn build_args(fds: &[OwnedFd]) -> (String, Vec<String>) {
    let prog = "Xwayland".to_string();
    let args = vec![
        "-terminate".to_string(),
        "-rootless".to_string(),
        "-verbose".to_string(),
        10.to_string(),
        "-displayfd".to_string(),
        fds[0].raw().to_string(),
        "-listenfd".to_string(),
        fds[1].raw().to_string(),
        "-wm".to_string(),
        fds[2].raw().to_string(),
    ];
    (prog, args)
}

async fn log_xwayland(state: Rc<State>, stderr: OwnedFd) {
    let res = Errno::tri(|| {
        uapi::fcntl_setfl(
            stderr.raw(),
            uapi::fcntl_getfl(stderr.raw())? | c::O_NONBLOCK,
        )?;
        Ok(())
    });
    if let Err(e) = res {
        log::error!("Could not set stderr fd to nonblock: {}", ErrorFmt(e));
        return;
    }
    let afd = match state.eng.fd(&Rc::new(stderr)) {
        Ok(f) => f,
        Err(e) => {
            log::error!(
                "Could not turn the stderr fd into an async fd: {}",
                ErrorFmt(e)
            );
            return;
        }
    };
    let mut buf = vec![];
    let mut buf2 = [0; 128];
    let mut done = false;
    while !done {
        if let Err(e) = afd.readable().await {
            log::error!(
                "Cannot wait for the xwayland stderr to become readable: {}",
                ErrorFmt(e)
            );
            return;
        }
        loop {
            match uapi::read(afd.raw(), &mut buf2[..]) {
                Ok(buf2) if buf2.len() > 0 => {
                    buf.extend_from_slice(buf2);
                }
                Ok(_) => {
                    done = true;
                    break;
                }
                Err(Errno(c::EAGAIN)) => {
                    break;
                }
                Err(e) => {
                    log::error!(
                        "Could not read from stderr fd: {}",
                        ErrorFmt(crate::utils::oserror::OsError::from(e))
                    );
                    return;
                }
            }
        }
        for line in buf.lines() {
            log::info!("Xwayland: {}", line.as_bstr());
        }
        buf.clear();
    }
}

pub enum XWaylandEvent {
    SurfaceCreated(Rc<WlSurface>),
    SurfaceDestroyed(WlSurfaceId),
    Configure(Rc<Xwindow>),
    Activate(Rc<XwindowData>),
    ActivateRoot,
    Close(Rc<XwindowData>),
}
