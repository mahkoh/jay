mod ptl_display;
mod ptl_render_ctx;
mod ptl_screencast;
mod ptr_gui;

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        dbus::{
            Dbus, DbusSocket, BUS_DEST, BUS_PATH, DBUS_NAME_FLAG_DO_NOT_QUEUE,
            DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER,
        },
        io_uring::IoUring,
        logger,
        pipewire::pw_con::PwCon,
        portal::{
            ptl_display::{watch_displays, PortalDisplay, PortalDisplayId},
            ptl_render_ctx::PortalRenderCtx,
            ptl_screencast::{add_screencast_dbus_members, ScreencastSession},
        },
        utils::{
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell,
            run_toplevel::RunToplevel, xrd::xrd,
        },
        wheel::Wheel,
        wire_dbus::org,
    },
    log::Level,
    std::{
        cell::Cell,
        rc::{Rc, Weak},
    },
    uapi::c,
};

const PORTAL_SUCCESS: u32 = 0;
#[allow(dead_code)]
const PORTAL_CANCELLED: u32 = 1;
const PORTAL_ENDED: u32 = 2;

pub fn run() {
    logger::Logger::install_stderr(Level::Trace);
    let xrd = match xrd() {
        Some(xrd) => xrd,
        _ => {
            fatal!("XDG_RUNTIME_DIR is not set");
        }
    };
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let wheel = Wheel::new(&eng, &ring).unwrap();
    let pw_con = PwCon::new(&eng, &ring).unwrap();
    let (_rtl_future, rtl) = RunToplevel::install(&eng);
    let dbus = Dbus::new(&eng, &ring, &rtl);
    let dbus = init_dbus_session(&dbus);
    let state = Rc::new(PortalState {
        xrd,
        ring,
        eng,
        wheel,
        pw_con,
        displays: Default::default(),
        watch_displays: Cell::new(None),
        dbus,
        screencasts: Default::default(),
        next_id: NumCell::new(1),
        render_ctxs: Default::default(),
    });
    let _root = {
        let obj = state
            .dbus
            .add_object("/org/freedesktop/portal/desktop")
            .unwrap();
        add_screencast_dbus_members(&state, &obj);
        obj
    };
    state
        .watch_displays
        .set(Some(state.eng.spawn(watch_displays(state.clone()))));
    state.ring.run().unwrap();
}

const UNIQUE_NAME: &str = "org.freedesktop.impl.portal.desktop.jay";

fn init_dbus_session(dbus: &Dbus) -> Rc<DbusSocket> {
    let session = match dbus.session() {
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
    watch_displays: Cell<Option<SpawnedFuture<()>>>,
    dbus: Rc<DbusSocket>,
    screencasts: CopyHashMap<String, Rc<ScreencastSession>>,
    next_id: NumCell<u32>,
    render_ctxs: CopyHashMap<c::dev_t, Weak<PortalRenderCtx>>,
}

impl PortalState {
    pub fn id<T: From<u32>>(&self) -> T {
        T::from(self.next_id.fetch_add(1))
    }
}
