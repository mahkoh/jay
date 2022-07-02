mod portal_display;
mod portal_render_ctx;
mod portal_screencast;

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
            portal_display::{watch_displays, PortalDisplay},
            portal_render_ctx::PortalRenderCtx,
            portal_screencast::PortalScreencastsState,
        },
        utils::{
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell,
            run_toplevel::RunToplevel, xrd::xrd,
        },
        wheel::Wheel,
        wire_dbus::org,
    },
    log::Level,
    portal_screencast::add_screencast_dbus_members,
    std::{
        cell::Cell,
        rc::{Rc, Weak},
    },
    uapi::c,
};

const PORTAL_SUCCESS: u32 = 0;
const PORTAL_CANCELLED: u32 = 1;
const PORTAL_ENDED: u32 = 2;

pub fn run() {
    run1();
}

// struct ClientNodeOwner {
//     node: Rc<PwClientNode>,
//     port: Rc<PwClientNodePort>,
//     state: Rc<PortalState>,
//     wl_buffers: RefCell<Vec<Rc<ShmBufferOwner>>>,
// }
//
// impl PwClientNodeOwner for ClientNodeOwner {
//     fn port_format_changed(&self, port: &Rc<PwClientNodePort>) {
//         let format = port.effective_format.get();
//         let mut bc = PwClientNodeBufferConfig::default();
//         bc.num_buffers = 3;
//         bc.planes = 1;
//         if let (Some(size), Some(format)) = (format.video_size, format.format) {
//             let stride = size.width * format.bpp;
//             bc.stride = Some(stride);
//             bc.size = Some(stride * size.height)
//         }
//         bc.align = 16;
//         bc.data_type = SPA_DATA_MemFd;
//         port.buffer_config.set(Some(bc));
//         self.node.send_port_update(port);
//     }
//
//     fn use_buffers(&self, port: &Rc<PwClientNodePort>) {
//         let shm = self.state.shm.get().unwrap();
//         let output = self.state.output.get().unwrap();
//         let buffers = port.buffers.borrow_mut();
//         let width = output.width.get().unwrap();
//         let height = output.height.get().unwrap();
//         let size = buffers.len() as i32 * width * height * 4;
//         let buf = uapi::memfd_create("buffer", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
//         uapi::ftruncate(buf.raw(), size as _).unwrap();
//         uapi::fcntl_add_seals(buf.raw(), c::F_SEAL_SHRINK | c::F_SEAL_SEAL).unwrap();
//         let fd = Rc::new(buf);
//         let pool = Rc::new(UsrWlShmPool {
//             id: shm.con.id(),
//             con: shm.con.clone(),
//             fd: fd.clone(),
//             size: Cell::new(size),
//         });
//         shm.request_create_pool(&pool);
//         shm.con.add_object(pool.clone());
//         let mut wl_buffers = vec![];
//         let mut pw_buffers = vec![];
//         let mut offset = 0;
//         for _ in buffers.deref() {
//             let buffer = Rc::new(UsrWlBuffer {
//                 id: shm.con.id(),
//                 con: shm.con.clone(),
//                 width,
//                 height,
//                 stride: width * 4,
//                 format: XRGB8888,
//                 owner: Default::default(),
//             });
//             let owner = Rc::new(ShmBufferOwner {
//                 buffer: buffer.clone(),
//             });
//             buffer.owner.set(Some(owner.clone()));
//             shm.con.add_object(buffer.clone());
//             pool.request_create_buffer(offset, &buffer);
//             pw_buffers.push(PwNodeBuffer {
//                 width,
//                 height,
//                 stride: width * 4,
//                 offset,
//                 fd: fd.clone(),
//             });
//             offset += width * height * 4;
//             wl_buffers.push(owner);
//         }
//         self.node.send_port_output_buffers(port, &pw_buffers);
//         *self.wl_buffers.borrow_mut() = wl_buffers;
//     }
//
//     fn start(self: Rc<Self>) {
//         let cast = Rc::new(OngoingScreencast {
//             state: self.state.clone(),
//             port: self.port.clone(),
//             idx: NumCell::new(0),
//             buffers: self.wl_buffers.borrow_mut().clone(),
//         });
//         cast.next_frame();
//     }
// }
//
// struct OngoingScreencast {
//     state: Rc<PortalState>,
//     port: Rc<PwClientNodePort>,
//     idx: NumCell<usize>,
//     buffers: Vec<Rc<ShmBufferOwner>>,
// }
//
// impl OngoingScreencast {
//     fn next_frame(self: &Rc<Self>) {
//         let idx = self.idx.fetch_add(1) % self.buffers.len();
//         let frame = Rc::new(UsrZwlrScreencopyFrame {
//             id: self.state.wl_con.id(),
//             con: self.state.wl_con.clone(),
//             owner: Default::default(),
//         });
//         let owner = Rc::new(ScreencopyFrameOwner {
//             frame: frame.clone(),
//             port: self.port.clone(),
//             buffer: self.buffers[idx].clone(),
//             idx,
//             cast: self.clone(),
//         });
//         frame.owner.set(Some(owner));
//         let manager = self.state.scp_manager.get().unwrap();
//         let output = self.state.output.get().unwrap();
//         manager.request_capture_output(&frame, &output.output);
//         self.state.wl_con.add_object(frame.clone());
//     }
// }

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

fn run1() {
    logger::Logger::install_stderr(Level::Trace);
    let xrd = match xrd() {
        Some(xrd) => xrd,
        _ => {
            log::error!("XDG_RUNTIME_DIR is not set");
            return;
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

struct PortalState {
    xrd: String,
    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,
    wheel: Rc<Wheel>,
    pw_con: Rc<PwCon>,
    displays: CopyHashMap<u32, Rc<PortalDisplay>>,
    watch_displays: Cell<Option<SpawnedFuture<()>>>,
    dbus: Rc<DbusSocket>,
    screencasts: PortalScreencastsState,
    next_id: NumCell<u32>,
    render_ctxs: CopyHashMap<c::dev_t, Weak<PortalRenderCtx>>,
}

// struct RegistryOwner {
//     registry: Rc<UsrWlRegistry>,
//     state: Rc<PortalState>,
// }
//
// impl UsrWlRegistryOwner for RegistryOwner {
//     fn global(&self, name: u32, interface: &str, version: u32) {
//         if interface == WlShm.name() {
//             let shm = Rc::new(UsrWlShm {
//                 id: self.registry.con.id(),
//                 con: self.registry.con.clone(),
//                 formats: Default::default(),
//             });
//             self.registry.request_bind(name, version, shm.deref());
//             self.registry.con.add_object(shm.clone());
//             self.state.shm.set(Some(shm));
//         } else if interface == WlOutput.name() {
//             if self.state.output.get().is_some() {
//                 return;
//             }
//             let output = Rc::new(UsrWlOutput {
//                 id: self.registry.con.id(),
//                 con: self.registry.con.clone(),
//                 owner: Default::default(),
//             });
//             let owner = Rc::new(OutputOwner {
//                 output: output.clone(),
//                 width: Cell::new(None),
//                 height: Cell::new(None),
//             });
//             output.owner.set(Some(owner.clone()));
//             self.registry.request_bind(name, version, output.deref());
//             self.registry.con.add_object(output);
//             self.state.output.set(Some(owner));
//         } else if interface == ZwlrScreencopyManagerV1.name() {
//             let manager = Rc::new(UsrZwlrScreencopyManager {
//                 id: self.registry.con.id(),
//                 con: self.registry.con.clone(),
//             });
//             self.registry.request_bind(name, version, manager.deref());
//             self.registry.con.add_object(manager.clone());
//             self.state.scp_manager.set(Some(manager));
//         }
//         log::info!("global: name={name}, interface={interface}, version={version}");
//     }
//
//     fn global_remove(&self, name: u32) {
//         log::info!("global_remove: name={name}");
//     }
// }
//
// struct OutputOwner {
//     output: Rc<UsrWlOutput>,
//     width: Cell<Option<i32>>,
//     height: Cell<Option<i32>>,
// }
//
// impl UsrWlOutputOwner for OutputOwner {
//     fn mode(&self, ev: &Mode) {
//         self.width.set(Some(ev.width));
//         self.height.set(Some(ev.height));
//     }
// }
//
// struct ScreencopyFrameOwner {
//     frame: Rc<UsrZwlrScreencopyFrame>,
//     port: Rc<PwClientNodePort>,
//     buffer: Rc<ShmBufferOwner>,
//     idx: usize,
//     cast: Rc<OngoingScreencast>,
// }
//
// impl UsrZwlrScreencopyFrameOwner for ScreencopyFrameOwner {
//     fn ready(&self, _ready: &Ready) {
//         unsafe {
//             for io in self.port.io_buffers.lock().values() {
//                 if io.read().status != SPA_STATUS_NEED_DATA {
//                     log::info!("status = {:?}", io.read().status);
//                     continue;
//                 }
//                 log::info!("buffer = {}", io.write().buffer_id);
//                 io.write().buffer_id = self.idx as _;
//                 io.write().status = SPA_STATUS_HAVE_DATA;
//             }
//             {
//                 let chunk = &self.port.buffers.borrow_mut()[self.idx].chunks[0];
//                 let chunk = chunk.write();
//                 chunk.flags = SpaChunkFlags::none();
//                 chunk.offset = 0;
//                 chunk.stride = self.buffer.buffer.stride as _;
//                 chunk.size = (self.buffer.buffer.stride * self.buffer.buffer.height) as _;
//             }
//         }
//         if let Some(wfd) = self.port.node.transport_out.get() {
//             let _ = uapi::eventfd_write(wfd.raw(), 1);
//         }
//         self.frame.request_destroy();
//         self.frame.con.remove_obj(self.frame.deref());
//         self.cast.next_frame();
//     }
//
//     fn buffer_done(&self) {
//         self.frame.request_copy(&self.buffer.buffer);
//     }
// }
//
// struct ShmBufferOwner {
//     buffer: Rc<UsrWlBuffer>,
// }
//
// impl UsrWlBufferOwner for ShmBufferOwner {}
