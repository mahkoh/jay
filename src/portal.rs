mod ptl_display;
mod ptl_render_ctx;
mod ptl_screencast;
mod ptr_gui;

use {
    crate::{
        async_engine::AsyncEngine,
        cli::GlobalArgs,
        dbus::{
            Dbus, DbusSocket, BUS_DEST, BUS_PATH, DBUS_NAME_FLAG_DO_NOT_QUEUE,
            DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER,
        },
        forker::ForkerError,
        io_uring::IoUring,
        logger::Logger,
        pipewire::pw_con::{PwCon, PwConHolder, PwConOwner},
        portal::{
            ptl_display::{watch_displays, PortalDisplay, PortalDisplayId},
            ptl_render_ctx::PortalRenderCtx,
            ptl_screencast::{add_screencast_dbus_members, ScreencastSession},
        },
        utils::{
            clone3::{fork_with_pidfd, Forked},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            line_logger::log_lines,
            numcell::NumCell,
            oserror::OsError,
            process_name::set_process_name,
            run_toplevel::RunToplevel,
            xrd::xrd,
        },
        version::VERSION,
        video::dmabuf::DmaBufIds,
        wheel::Wheel,
        wire_dbus::org,
    },
    log::Level,
    std::{
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

const PORTAL_SUCCESS: u32 = 0;
#[allow(dead_code)]
const PORTAL_CANCELLED: u32 = 1;
#[allow(dead_code)]
const PORTAL_ENDED: u32 = 2;

pub fn run_freestanding(global: GlobalArgs) {
    let logger = Logger::install_stderr(global.log_level.into());
    run(logger);
}

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("Could not create pipe")]
    CreatePipe(#[source] OsError),
    #[error("Could not fork")]
    Fork(#[source] ForkerError),
}

pub struct PortalStartup {
    logs: Rc<OwnedFd>,
    pid: c::pid_t,
    pidfd: Rc<OwnedFd>,
}

impl PortalStartup {
    pub async fn spawn(self, eng: Rc<AsyncEngine>, ring: Rc<IoUring>, logger: Arc<Logger>) {
        let f1 = eng.spawn({
            let ring = ring.clone();
            async move {
                if let Err(e) = ring.readable(&self.pidfd).await {
                    log::error!(
                        "Could not wait for portal pidfd to become readable: {}",
                        ErrorFmt(e)
                    );
                    return;
                }
                let (_, status) = match uapi::waitpid(self.pid, 0) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!(
                            "Could not retrieve exit status of portal ({}): {}",
                            self.pid,
                            ErrorFmt(OsError::from(e))
                        );
                        return;
                    }
                };
                let status = uapi::WEXITSTATUS(status);
                if status != 0 {
                    log::error!("Portal exited with non-0 exit code: {status}");
                }
            }
        });
        let f2 = eng.spawn({
            let ring = ring.clone();
            let logger = logger.clone();
            async move {
                let res = log_lines(&ring, &self.logs, |left, right| {
                    logger.write_raw(left);
                    logger.write_raw(right);
                    logger.write_raw(b" (portal)\n");
                })
                .await;
                if let Err(e) = res {
                    log::error!("Could not read portal logs: {}", ErrorFmt(e));
                }
            }
        });
        f1.await;
        f2.await;
    }
}

pub fn run_from_compositor(level: Level) -> Result<PortalStartup, PortalError> {
    let (read, write) = match uapi::pipe2(c::O_CLOEXEC) {
        Ok(p) => p,
        Err(e) => return Err(PortalError::CreatePipe(e.into())),
    };
    let fork = match fork_with_pidfd(false) {
        Ok(f) => f,
        Err(e) => return Err(PortalError::Fork(e)),
    };
    match fork {
        Forked::Parent { pidfd, pid } => Ok(PortalStartup {
            logs: Rc::new(read),
            pid,
            pidfd: Rc::new(pidfd),
        }),
        Forked::Child { .. } => {
            drop(read);
            let logger = Logger::install_pipe(write, level);
            run(logger);
            std::process::exit(0);
        }
    }
}

fn run(logger: Arc<Logger>) {
    let eng = AsyncEngine::new();
    let ring = match IoUring::new(&eng, 32) {
        Ok(r) => r,
        Err(e) => {
            fatal!("Could not create an IO-uring: {}", ErrorFmt(e));
        }
    };
    let _f = eng.spawn(run_async(eng.clone(), ring.clone(), logger));
    if let Err(e) = ring.run() {
        fatal!("The IO-uring returned an error: {}", ErrorFmt(e));
    }
}

async fn run_async(eng: Rc<AsyncEngine>, ring: Rc<IoUring>, logger: Arc<Logger>) {
    let (_rtl_future, rtl) = RunToplevel::install(&eng);
    let dbus = Dbus::new(&eng, &ring, &rtl);
    let dbus = init_dbus_session(&dbus, logger).await;
    let xrd = match xrd() {
        Some(xrd) => xrd,
        _ => {
            fatal!("XDG_RUNTIME_DIR is not set");
        }
    };
    let wheel = match Wheel::new(&eng, &ring) {
        Ok(w) => w,
        Err(e) => {
            fatal!("Could not create a timer wheel: {}", ErrorFmt(e));
        }
    };
    let pw_con = match PwConHolder::new(&eng, &ring).await {
        Ok(p) => p,
        Err(e) => {
            fatal!("Could not connect to pipewire: {}", ErrorFmt(e));
        }
    };
    let state = Rc::new(PortalState {
        xrd,
        ring,
        eng,
        wheel,
        pw_con: pw_con.con.clone(),
        displays: Default::default(),
        dbus,
        screencasts: Default::default(),
        next_id: NumCell::new(1),
        render_ctxs: Default::default(),
        dma_buf_ids: Default::default(),
    });
    let _root = {
        let obj = state
            .dbus
            .add_object("/org/freedesktop/portal/desktop")
            .unwrap();
        add_screencast_dbus_members(&state, &obj);
        obj
    };
    state.pw_con.owner.set(Some(state.clone()));
    watch_displays(state.clone()).await;
}

const UNIQUE_NAME: &str = "org.freedesktop.impl.portal.desktop.jay";

async fn init_dbus_session(dbus: &Dbus, logger: Arc<Logger>) -> Rc<DbusSocket> {
    let session = match dbus.session().await {
        Ok(s) => s,
        Err(e) => {
            fatal!("Could not connect to dbus session daemon: {}", ErrorFmt(e));
        }
    };
    let rv = session
        .call_async(
            BUS_DEST,
            BUS_PATH,
            org::freedesktop::dbus::RequestName {
                name: UNIQUE_NAME.into(),
                flags: DBUS_NAME_FLAG_DO_NOT_QUEUE,
            },
        )
        .await;
    match rv {
        Ok(r) if r.get().rv == DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER => {
            log::info!("Acquired unique name {}", UNIQUE_NAME);
            logger.redirect("portal");
            log::info!("version = {VERSION}");
            let fork = match fork_with_pidfd(false) {
                Ok(f) => f,
                Err(e) => fatal!("Could not fork: {}", ErrorFmt(e)),
            };
            match fork {
                Forked::Parent { .. } => std::process::exit(0),
                Forked::Child { .. } => {
                    if let Err(e) = uapi::setsid() {
                        log::error!("setsid failed: {}", ErrorFmt(OsError::from(e)));
                    }
                }
            }
            set_process_name("jay portal");
            session
        }
        Ok(_) => {
            log::info!("Portal is already running");
            std::process::exit(0);
        }
        Err(e) => {
            fatal!(
                "Could not communicate with the session bus: {}",
                ErrorFmt(e)
            );
        }
    }
}

struct PortalState {
    xrd: String,
    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,
    wheel: Rc<Wheel>,
    pw_con: Rc<PwCon>,
    displays: CopyHashMap<PortalDisplayId, Rc<PortalDisplay>>,
    dbus: Rc<DbusSocket>,
    screencasts: CopyHashMap<String, Rc<ScreencastSession>>,
    next_id: NumCell<u32>,
    render_ctxs: CopyHashMap<c::dev_t, Weak<PortalRenderCtx>>,
    dma_buf_ids: Rc<DmaBufIds>,
}

impl PortalState {
    pub fn id<T: From<u32>>(&self) -> T {
        T::from(self.next_id.fetch_add(1))
    }
}

impl PwConOwner for PortalState {
    fn killed(&self) {
        fatal!("The pipewire connection has been closed");
    }
}
