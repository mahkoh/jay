mod xsocket;
mod xwm;

use crate::client::ClientError;
use crate::forker::ForkerProxy;
use crate::ifs::wl_surface::xwindow::Xwindow;
use crate::ifs::wl_surface::WlSurface;
use crate::utils::tri::Try;
use crate::wire::WlSurfaceId;
use crate::xwayland::xsocket::allocate_socket;
use crate::xwayland::xwm::Wm;
use crate::{AsyncError, AsyncQueue, ErrorFmt, ForkerError, State};
use bstr::ByteSlice;
use std::error::Error;
use std::num::ParseIntError;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, pipe2, Errno, OwnedFd};

#[derive(Debug, Error)]
enum XWaylandError {
    #[error("Could not create a wayland socket")]
    SocketFailed(#[source] std::io::Error),
    #[error("/tmp/.X11-unix does not exist")]
    MissingSocketDir,
    #[error("Could not stat /tmp/.X11-unix")]
    StatSocketDir(#[source] std::io::Error),
    #[error("/tmp/.X11-unix is not a directory")]
    NotASocketDir,
    #[error("/tmp/.X11-unix is writable")]
    SocketDirNotWritable,
    #[error("Could not write to the lock file")]
    WriteLockFile(#[source] std::io::Error),
    #[error("Could not open the lock file for reading")]
    ReadLockFile(#[source] std::io::Error),
    #[error("The lock file does not contain a PID")]
    NotALockFile(#[source] ParseIntError),
    #[error("The socket is already in use")]
    AlreadyInUse,
    #[error("Could not bind the socket to an address")]
    BindFailed(#[source] std::io::Error),
    #[error("All X displays in the range 0..1000 are already in use")]
    AddressesInUse,
    #[error("The async engine returned an error")]
    AsyncError(#[from] AsyncError),
    #[error("pipe(2) failed")]
    Pipe(#[source] std::io::Error),
    #[error("dupfd(2) failed")]
    Dupfd(#[source] std::io::Error),
    #[error("socketpair(2) failed")]
    Socketpair(#[source] std::io::Error),
    #[error("Could not start Xwayland")]
    ExecFailed(#[source] ForkerError),
    #[error("Could not load the atoms")]
    LoadAtoms(#[source] Box<dyn Error>),
    #[error("Could not connect to Xwayland")]
    Connect(#[source] Box<dyn Error>),
    #[error("Could not create a window manager")]
    CreateWm(#[source] Box<Self>),
    #[error("Could not select the root events")]
    SelectRootEvents(#[source] Box<dyn Error>),
    #[error("Could not create the WM window")]
    CreateXWindow(#[source] Box<dyn Error>),
    #[error("Could not acquire a selection")]
    SelectionOwner(#[source] Box<dyn Error>),
    #[error("Could not load the resource database")]
    ResourceDatabase(#[source] Box<dyn Error>),
    #[error("Could not acquire a cursor handle")]
    CursorHandle(#[source] Box<dyn Error>),
    #[error("Could not load the default cursor")]
    LoadCursor(#[source] Box<dyn Error>),
    #[error("Could not set the cursor of the root window")]
    SetCursor(#[source] Box<dyn Error>),
    #[error("composite_redirect_subwindows failed")]
    CompositeRedirectSubwindows(#[source] Box<dyn Error>),
    #[error("Could not spawn the Xwayland client")]
    SpawnClient(#[source] ClientError),
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
        }
        forker.setenv(b"DISPLAY", format!(":{}", xsocket.id).as_bytes());
        log::info!("Allocated display :{} for Xwayland", xsocket.id);
        log::info!("Waiting for connection attempt");
        let res = XWaylandError::tria(async {
            let _ = state.eng.fd(&socket)?.readable().await;
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
        forker.unsetenv(b"DISPLAY");
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
    let queue = Rc::new(AsyncQueue::new());
    let client = state
        .clients
        .spawn2(client_id, state, client1, 9999, 9999, Some(queue.clone()));
    let client = match client {
        Ok(c) => c,
        Err(e) => return Err(XWaylandError::SpawnClient(e)),
    };
    let _ = state.eng.fd(&Rc::new(dfdread))?.readable().await;
    let wm = match Wm::get(state, client, wm1, queue.clone()) {
        Ok(w) => w,
        Err(e) => return Err(XWaylandError::CreateWm(Box::new(e))),
    };
    let wm = state.eng.spawn(wm.run());
    let _ = state.eng.fd(&Rc::new(pidfd))?.readable().await;
    drop(wm);
    queue.clear();
    stderr_read.await;
    Ok(())
}

pub fn build_args(fds: &[OwnedFd]) -> (String, Vec<String>) {
    let prog = "Xwayland".to_string();
    let mut args = vec![];
    args.push("-terminate".to_string());
    args.push("-rootless".to_string());
    args.push("-verbose".to_string());
    args.push(10.to_string());
    args.push("-displayfd".to_string());
    args.push(fds[0].raw().to_string());
    args.push("-listenfd".to_string());
    args.push(fds[1].raw().to_string());
    args.push("-wm".to_string());
    args.push(fds[2].raw().to_string());
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
        let _ = afd.readable().await;
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
                        ErrorFmt(std::io::Error::from(e))
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
}
