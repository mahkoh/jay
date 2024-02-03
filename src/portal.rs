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
        io_uring::IoUring,
        logger,
        pipewire::pw_con::{PwCon, PwConHolder, PwConOwner},
        portal::{
            ptl_display::{watch_displays, PortalDisplay, PortalDisplayId},
            ptl_render_ctx::PortalRenderCtx,
            ptl_screencast::{add_screencast_dbus_members, ScreencastSession},
        },
        utils::{
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell,
            run_toplevel::RunToplevel, xrd::xrd,
        },
        video::drm::wait_for_syncobj::WaitForSyncObj,
        wheel::Wheel,
        wire_dbus::org,
    },
    std::rc::{Rc, Weak},
    uapi::c,
};

const PORTAL_SUCCESS: u32 = 0;
#[allow(dead_code)]
const PORTAL_CANCELLED: u32 = 1;
#[allow(dead_code)]
const PORTAL_ENDED: u32 = 2;

pub fn run(global: GlobalArgs) {
    logger::Logger::install_stderr(global.log_level.into());
    let eng = AsyncEngine::new();
    let ring = match IoUring::new(&eng, 32) {
        Ok(r) => r,
        Err(e) => {
            fatal!("Could not create an IO-uring: {}", ErrorFmt(e));
        }
    };
    let _f = eng.spawn(run_async(eng.clone(), ring.clone()));
    if let Err(e) = ring.run() {
        fatal!("The IO-uring returned an error: {}", ErrorFmt(e));
    }
}

async fn run_async(eng: Rc<AsyncEngine>, ring: Rc<IoUring>) {
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
    let (_rtl_future, rtl) = RunToplevel::install(&eng);
    let dbus = Dbus::new(&eng, &ring, &rtl);
    let dbus = init_dbus_session(&dbus).await;
    let wait_for_sync_obj = Rc::new(WaitForSyncObj::new(&ring, &eng));
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
        wait_for_sync_obj,
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

async fn init_dbus_session(dbus: &Dbus) -> Rc<DbusSocket> {
    let session = match dbus.session().await {
        Ok(s) => s,
        Err(e) => {
            fatal!("Could not connect to dbus session daemon: {}", ErrorFmt(e));
        }
    };
    session.call(
        BUS_DEST,
        BUS_PATH,
        org::freedesktop::dbus::RequestName {
            name: UNIQUE_NAME.into(),
            flags: DBUS_NAME_FLAG_DO_NOT_QUEUE,
        },
        |rv| match rv {
            Ok(r) if r.rv == DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER => {
                log::info!("Acquired unique name {}", UNIQUE_NAME);
                return;
            }
            Ok(r) => {
                fatal!("Could not acquire unique name {}: {}", UNIQUE_NAME, r.rv);
            }
            Err(e) => {
                fatal!(
                    "Could not communicate with the session bus: {}",
                    ErrorFmt(e)
                );
            }
        },
    );
    session
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
    wait_for_sync_obj: Rc<WaitForSyncObj>,
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
