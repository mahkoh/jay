mod xsocket;
mod xwm;

use {
    crate::{
        client::{ClientCaps, ClientError},
        compositor::DISPLAY,
        forker::{ForkerError, ForkerProxy},
        ifs::{
            ipc::{DataOfferId, DataSourceId, IpcLocation, x_data_offer::XDataOffer},
            wl_seat::SeatId,
            wl_surface::x_surface::xwindow::XwindowData,
        },
        io_uring::IoUringError,
        security_context_acceptor::AcceptorMetadata,
        state::State,
        user_session::import_environment,
        utils::{
            buf::Buf, errorfmt::ErrorFmt, line_logger::log_lines, on_drop::OnDrop, oserror::OsError,
        },
        wire::WlSurfaceId,
        xcon::XconError,
        xwayland::{
            xsocket::allocate_socket,
            xwm::{Wm, XwmShared},
        },
    },
    bstr::ByteSlice,
    std::{num::ParseIntError, rc::Rc},
    thiserror::Error,
    uapi::{OwnedFd, c, pipe2},
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
    #[error("The io-uring returned an error")]
    RingError(#[from] IoUringError),
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
    #[error("Could not create a window to manage a selection")]
    CreateSelectionWindow(#[source] XconError),
    #[error("Could not watch selection changes")]
    WatchSelection(#[source] XconError),
    #[error("Could not enable the xfixes extension")]
    XfixesQueryVersion(#[source] XconError),
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
        if state.backend.get().import_environment() {
            import_environment(&state, DISPLAY, &display).await;
        }
        if let Err(e) = state.ring.readable(&socket).await {
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
    let wm = uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0);
    let (wm1, wm2) = match wm {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::Socketpair(e.into())),
    };
    let client = uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0);
    let (client1, client2) = match client {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::Socketpair(e.into())),
    };
    let stderr_read = state
        .eng
        .spawn("log Xwayland", log_xwayland(state.clone(), stderr_read));
    let pidfd = forker
        .xwayland(
            &state,
            Rc::new(stderr_write),
            Rc::new(dfdwrite),
            socket,
            Rc::new(wm2),
            Rc::new(client2),
        )
        .await;
    let (pidfd, pid) = match pidfd {
        Ok(p) => p,
        Err(e) => return Err(XWaylandError::ExecFailed(e)),
    };
    let client_id = state.clients.id();
    let client = state.clients.spawn2(
        client_id,
        state,
        Rc::new(client1),
        uapi::getuid(),
        pid,
        ClientCaps::all(),
        false,
        true,
        &Rc::new(AcceptorMetadata::default()),
    );
    let client = match client {
        Ok(c) => c,
        Err(e) => return Err(XWaylandError::SpawnClient(e)),
    };
    state.update_xwayland_wire_scale();
    state.ring.readable(&Rc::new(dfdread)).await?;
    state.xwayland.queue.clear();
    state.xwayland.pidfd.set(Some(pidfd.clone()));
    let _remove_pidfd = OnDrop(|| {
        state.xwayland.pidfd.take();
    });
    {
        let shared = Rc::new(XwmShared::default());
        let wm = match Wm::get(state, client, wm1, &shared).await {
            Ok(w) => w,
            Err(e) => return Err(XWaylandError::CreateWm(Box::new(e))),
        };
        let _wm = state.eng.spawn("XWM", wm.run());
        state.ring.readable(&pidfd).await?;
    }
    state.xwayland.queue.clear();
    state.xwayland.windows.clear();
    stderr_read.await;
    Ok(())
}

const PROG: &str = "Xwayland";
const ENABLE_EI_PORTAL: &str = "-enable-ei-portal";

pub async fn build_args(state: &State, forker: &ForkerProxy) -> (String, Vec<String>) {
    let prog = PROG.to_string();
    let mut args = vec![
        "-terminate".to_string(),
        "-rootless".to_string(),
        "-verbose".to_string(),
        10.to_string(),
        "-displayfd".to_string(),
        "3".to_string(),
        "-listenfd".to_string(),
        "4".to_string(),
        "-wm".to_string(),
        "5".to_string(),
    ];
    let features = detect_features(state, forker).await;
    if features.ei_portal {
        args.push(ENABLE_EI_PORTAL.to_string());
    }
    (prog, args)
}

#[derive(Default, Debug)]
struct XwaylandFeatures {
    ei_portal: bool,
}

async fn detect_features(state: &State, forker: &ForkerProxy) -> XwaylandFeatures {
    let mut features = Default::default();
    let Ok((read, write)) = pipe2(c::O_CLOEXEC) else {
        return features;
    };
    forker.spawn(
        PROG.to_string(),
        vec!["-help".to_string()],
        vec![],
        vec![(2, Rc::new(write))],
    );
    let read = Rc::new(read);
    let mut help = Vec::new();
    let mut buf = Buf::new(1024);
    loop {
        match state.ring.read(&read, buf.clone()).await {
            Ok(0) => break,
            Ok(n) => help.extend_from_slice(&buf[..n]),
            Err(_) => return features,
        }
    }
    if help.as_bstr().contains_str(ENABLE_EI_PORTAL) {
        features.ei_portal = true;
    }
    features
}

async fn log_xwayland(state: Rc<State>, stderr: OwnedFd) {
    let stderr = Rc::new(stderr);
    let res = log_lines(&state.ring, &stderr, |left, right| {
        log::info!("Xwayland: {}{}", left.as_bstr(), right.as_bstr());
    })
    .await;
    if let Err(e) = res {
        log::error!("Could not read from stderr fd: {}", ErrorFmt(e));
    }
}

pub enum XWaylandEvent {
    SurfaceCreated(WlSurfaceId),
    SurfaceSerialAssigned(WlSurfaceId),
    SurfaceDestroyed(WlSurfaceId, Option<u64>),
    Configure(Rc<XwindowData>),
    Activate(Rc<XwindowData>),
    ActivateRoot,
    Close(Rc<XwindowData>),
    #[expect(dead_code)]
    SeatChanged,

    IpcCancelSource {
        location: IpcLocation,
        seat: SeatId,
        source: DataSourceId,
    },
    IpcSendSource {
        location: IpcLocation,
        seat: SeatId,
        source: DataSourceId,
        mime_type: String,
        fd: Rc<OwnedFd>,
    },
    IpcSetOffer {
        location: IpcLocation,
        seat: SeatId,
        offer: Rc<XDataOffer>,
    },
    IpcSetSelection {
        location: IpcLocation,
        seat: SeatId,
        offer: Option<Rc<XDataOffer>>,
    },
    IpcAddOfferMimeType {
        location: IpcLocation,
        seat: SeatId,
        offer: DataOfferId,
        mime_type: String,
    },
}
