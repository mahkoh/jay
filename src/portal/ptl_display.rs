use {
    crate::{
        gfx_apis::create_gfx_context,
        ifs::wl_seat::POINTER,
        object::Version,
        portal::{
            ptl_render_ctx::PortalRenderCtx, ptl_screencast::ScreencastSession,
            ptr_gui::WindowData, PortalState,
        },
        utils::{
            bitflags::BitflagsExt, clonecell::CloneCell, copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt, hash_map_ext::HashMapExt, oserror::OsError,
        },
        video::drm::Drm,
        wire::{
            wl_pointer, JayCompositor, WlCompositor, WlOutput, WlSeat, WlSurfaceId,
            WpFractionalScaleManagerV1, WpViewporter, ZwlrLayerShellV1, ZwpLinuxDmabufV1,
        },
        wl_usr::{
            usr_ifs::{
                usr_jay_compositor::UsrJayCompositor,
                usr_jay_output::{UsrJayOutput, UsrJayOutputOwner},
                usr_jay_pointer::UsrJayPointer,
                usr_jay_render_ctx::UsrJayRenderCtxOwner,
                usr_linux_dmabuf::UsrLinuxDmabuf,
                usr_wl_compositor::UsrWlCompositor,
                usr_wl_output::{UsrWlOutput, UsrWlOutputOwner},
                usr_wl_pointer::{UsrWlPointer, UsrWlPointerOwner},
                usr_wl_registry::{UsrWlRegistry, UsrWlRegistryOwner},
                usr_wl_seat::{UsrWlSeat, UsrWlSeatOwner},
                usr_wlr_layer_shell::UsrWlrLayerShell,
                usr_wp_fractional_scale_manager::UsrWpFractionalScaleManager,
                usr_wp_viewporter::UsrWpViewporter,
            },
            UsrCon, UsrConOwner,
        },
    },
    ahash::AHashMap,
    jay_config::video::GfxApi,
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        os::unix::ffi::OsStrExt,
        rc::Rc,
        str::FromStr,
    },
    uapi::{c, AsUstr, OwnedFd},
};

struct PortalDisplayPrelude {
    con: Rc<UsrCon>,
    state: Rc<PortalState>,
    registry: Rc<UsrWlRegistry>,
    globals: RefCell<AHashMap<String, Vec<(u32, u32)>>>,
}

shared_ids!(PortalDisplayId);
pub struct PortalDisplay {
    pub id: PortalDisplayId,
    pub con: Rc<UsrCon>,
    pub(super) state: Rc<PortalState>,
    registry: Rc<UsrWlRegistry>,
    pub dmabuf: CloneCell<Option<Rc<UsrLinuxDmabuf>>>,

    pub jc: Rc<UsrJayCompositor>,
    pub ls: Rc<UsrWlrLayerShell>,
    pub comp: Rc<UsrWlCompositor>,
    pub fsm: Rc<UsrWpFractionalScaleManager>,
    pub vp: Rc<UsrWpViewporter>,
    pub render_ctx: CloneCell<Option<Rc<PortalRenderCtx>>>,

    pub outputs: CopyHashMap<u32, Rc<PortalOutput>>,
    pub seats: CopyHashMap<u32, Rc<PortalSeat>>,

    pub windows: CopyHashMap<WlSurfaceId, Rc<WindowData>>,
    pub screencasts: CopyHashMap<String, Rc<ScreencastSession>>,
}

pub struct PortalOutput {
    pub global_id: u32,
    pub dpy: Rc<PortalDisplay>,
    pub wl: Rc<UsrWlOutput>,
    pub jay: Rc<UsrJayOutput>,
}

pub struct PortalSeat {
    pub global_id: u32,
    pub dpy: Rc<PortalDisplay>,
    pub wl: Rc<UsrWlSeat>,
    pub jay_pointer: Rc<UsrJayPointer>,
    pub pointer: CloneCell<Option<Rc<UsrWlPointer>>>,
    pub name: RefCell<String>,
    pub capabilities: Cell<u32>,
    pub pointer_focus: CloneCell<Option<Rc<WindowData>>>,
}

impl UsrWlSeatOwner for PortalSeat {
    fn name(&self, name: &str) {
        *self.name.borrow_mut() = name.to_string();
    }

    fn capabilities(self: Rc<Self>, value: u32) {
        let old = self.capabilities.replace(value);
        if old.contains(POINTER) != value.contains(POINTER) {
            if old.contains(POINTER) {
                if let Some(pointer) = self.pointer.take() {
                    pointer.con.remove_obj(pointer.deref());
                }
            } else {
                let pointer = self.wl.get_pointer();
                pointer.owner.set(Some(self.clone()));
                self.pointer.set(Some(pointer));
            }
        }
    }
}

impl UsrWlPointerOwner for PortalSeat {
    fn enter(&self, ev: &wl_pointer::Enter) {
        if let Some(window) = self.dpy.windows.get(&ev.surface) {
            self.pointer_focus.set(Some(window.clone()));
            window.motion(self, ev.surface_x, ev.surface_y, true);
        }
    }

    fn leave(&self, _ev: &wl_pointer::Leave) {
        self.pointer_focus.take();
    }

    fn motion(&self, ev: &wl_pointer::Motion) {
        if let Some(window) = self.pointer_focus.get() {
            window.motion(self, ev.surface_x, ev.surface_y, false);
        }
    }

    fn button(&self, ev: &wl_pointer::Button) {
        if let Some(window) = self.pointer_focus.get() {
            window.button(self, ev.button, ev.state);
        }
    }
}

impl UsrWlRegistryOwner for PortalDisplayPrelude {
    fn global(self: Rc<Self>, name: u32, interface: &str, version: u32) {
        self.globals
            .borrow_mut()
            .entry(interface.to_string())
            .or_default()
            .push((name, version));
    }
}

impl UsrJayRenderCtxOwner for PortalDisplay {
    fn no_device(&self) {
        self.render_ctx.take();
    }

    fn device(&self, fd: Rc<OwnedFd>) {
        self.render_ctx.take();
        let dev_id = match uapi::fstat(fd.raw()) {
            Ok(s) => s.st_rdev,
            Err(e) => {
                log::error!("Could not fstat display device: {}", ErrorFmt(e));
                return;
            }
        };
        if let Some(ctx) = self.state.render_ctxs.get(&dev_id) {
            if let Some(ctx) = ctx.upgrade() {
                self.render_ctx.set(Some(ctx));
            }
        }
        if self.render_ctx.is_none() {
            let drm = Drm::open_existing(fd);
            let ctx =
                match create_gfx_context(&self.state.eng, &self.state.ring, &drm, GfxApi::OpenGl) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!(
                            "Could not create render context from drm device: {}",
                            ErrorFmt(e)
                        );
                        return;
                    }
                };
            let ctx = Rc::new(PortalRenderCtx {
                _dev_id: dev_id,
                ctx,
            });
            self.render_ctx.set(Some(ctx.clone()));
            self.state.render_ctxs.set(dev_id, Rc::downgrade(&ctx));
        }
    }
}

impl UsrConOwner for PortalDisplay {
    fn killed(&self) {
        log::info!("Removing display {}", self.id);
        for sc in self.screencasts.lock().drain_values() {
            sc.kill();
        }
        self.windows.clear();
        self.state.displays.remove(&self.id);
    }
}

impl UsrWlRegistryOwner for PortalDisplay {
    fn global(self: Rc<Self>, name: u32, interface: &str, version: u32) {
        if interface == WlOutput.name() {
            add_output(&self, name, version);
        } else if interface == WlSeat.name() {
            add_seat(&self, name, version);
        } else if interface == ZwpLinuxDmabufV1.name() {
            let ls = Rc::new(UsrLinuxDmabuf {
                id: self.con.id(),
                con: self.con.clone(),
                owner: Default::default(),
                version: Version(version.min(5)),
            });
            self.con.add_object(ls.clone());
            self.registry.request_bind(name, version, ls.deref());
            self.dmabuf.set(Some(ls));
        }
    }
}

impl UsrJayOutputOwner for PortalOutput {
    fn destroyed(&self) {
        log::info!(
            "Display {}: Output {} removed",
            self.dpy.con.server_id,
            self.global_id,
        );
        self.dpy.outputs.remove(&self.global_id);
        self.dpy.con.remove_obj(self.wl.deref());
        self.dpy.con.remove_obj(self.jay.deref());
    }
}

impl UsrWlOutputOwner for PortalOutput {}

async fn maybe_add_display(state: &Rc<PortalState>, name: &str) {
    let tail = match name.strip_prefix("wayland-") {
        Some(t) => t,
        _ => return,
    };
    let head = match tail.strip_suffix(".jay") {
        Some(h) => h,
        _ => return,
    };
    let num = match u32::from_str(head) {
        Ok(n) => n,
        _ => return,
    };
    let path = format!("{}/{}", state.xrd, name);
    let con = match UsrCon::new(
        &state.ring,
        &state.wheel,
        &state.eng,
        &state.dma_buf_ids,
        &path,
        num,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "Could not connect to wayland display {}: {}",
                name,
                ErrorFmt(e)
            );
            return;
        }
    };
    let registry = con.get_registry();
    let dpy = Rc::new(PortalDisplayPrelude {
        con: con.clone(),
        state: state.clone(),
        registry,
        globals: Default::default(),
    });
    dpy.registry.owner.set(Some(dpy.clone()));
    con.sync(move || {
        finish_display_connect(dpy);
    });
    log::info!("Connected to wayland display {num}: {name}");
}

fn finish_display_connect(dpy: Rc<PortalDisplayPrelude>) {
    let mut jc_opt = None;
    let mut ls_opt = None;
    let mut fsm_opt = None;
    let mut comp_opt = None;
    let mut vp_opt = None;
    let mut dmabuf_opt = None;
    let mut outputs = vec![];
    let mut seats = vec![];
    for (interface, instances) in dpy.globals.borrow_mut().deref() {
        for &(name, version) in instances {
            if interface == JayCompositor.name() {
                let jc = Rc::new(UsrJayCompositor {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    owner: Default::default(),
                    caps: Default::default(),
                    version: Version(version.min(4)),
                });
                dpy.con.add_object(jc.clone());
                dpy.registry.request_bind(name, version, jc.deref());
                jc_opt = Some(jc);
            } else if interface == WpFractionalScaleManagerV1.name() {
                let ls = Rc::new(UsrWpFractionalScaleManager {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    version: Version(version.min(1)),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                fsm_opt = Some(ls);
            } else if interface == ZwlrLayerShellV1.name() {
                let ls = Rc::new(UsrWlrLayerShell {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    version: Version(version.min(5)),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                ls_opt = Some(ls);
            } else if interface == WpViewporter.name() {
                let ls = Rc::new(UsrWpViewporter {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    version: Version(version.min(1)),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                vp_opt = Some(ls);
            } else if interface == WlCompositor.name() {
                let ls = Rc::new(UsrWlCompositor {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    version: Version(version.min(6)),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                comp_opt = Some(ls);
            } else if interface == ZwpLinuxDmabufV1.name() {
                let ls = Rc::new(UsrLinuxDmabuf {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    owner: Default::default(),
                    version: Version(version.min(5)),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                dmabuf_opt = Some(ls);
            } else if interface == WlOutput.name() {
                outputs.push((name, version));
            } else if interface == WlSeat.name() {
                seats.push((name, version));
            }
        }
    }
    macro_rules! get {
        ($opt:expr, $ty:expr) => {
            match $opt {
                Some(c) => c,
                _ => {
                    log::error!("Compositor did not advertise a {}", $ty.name());
                    dpy.con.kill();
                    return;
                }
            }
        };
    }
    let jc = get!(jc_opt, JayCompositor);
    let ls = get!(ls_opt, ZwlrLayerShellV1);
    let comp = get!(comp_opt, WlCompositor);
    let fsm = get!(fsm_opt, WpFractionalScaleManagerV1);
    let vp = get!(vp_opt, WpViewporter);

    let dpy = Rc::new(PortalDisplay {
        id: dpy.state.id(),
        con: dpy.con.clone(),
        state: dpy.state.clone(),
        registry: dpy.registry.clone(),
        dmabuf: CloneCell::new(dmabuf_opt),
        jc,
        outputs: Default::default(),
        render_ctx: Default::default(),
        seats: Default::default(),
        ls,
        comp,
        fsm,
        vp,
        windows: Default::default(),
        screencasts: Default::default(),
    });

    dpy.state.displays.set(dpy.id, dpy.clone());
    dpy.con.owner.set(Some(dpy.clone()));
    dpy.registry.owner.set(Some(dpy.clone()));

    let jrc = dpy.jc.get_render_context();
    jrc.owner.set(Some(dpy.clone()));

    for (name, version) in outputs {
        add_output(&dpy, name, version);
    }
    for (name, version) in seats {
        add_seat(&dpy, name, version);
    }
    log::info!("Display {} initialized", dpy.id);
}

fn add_seat(dpy: &Rc<PortalDisplay>, name: u32, version: u32) {
    let wl = Rc::new(UsrWlSeat {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
        version: Version(version.min(9)),
    });
    dpy.con.add_object(wl.clone());
    dpy.registry.request_bind(name, version, wl.deref());
    let jay_pointer = dpy.jc.get_pointer(&wl);
    let js = Rc::new(PortalSeat {
        global_id: name,
        dpy: dpy.clone(),
        wl,
        jay_pointer,
        pointer: Default::default(),
        name: RefCell::new("".to_string()),
        capabilities: Cell::new(0),
        pointer_focus: Default::default(),
    });
    js.wl.owner.set(Some(js.clone()));
    dpy.seats.set(name, js);
}

fn add_output(dpy: &Rc<PortalDisplay>, name: u32, version: u32) {
    let wl = Rc::new(UsrWlOutput {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
        version: Version(version.min(4)),
    });
    dpy.con.add_object(wl.clone());
    dpy.registry.request_bind(name, version, wl.deref());
    let jo = dpy.jc.get_output(&wl);
    let po = Rc::new(PortalOutput {
        global_id: name,
        dpy: dpy.clone(),
        wl: wl.clone(),
        jay: jo.clone(),
    });
    po.wl.owner.set(Some(po.clone()));
    po.jay.owner.set(Some(po.clone()));
    dpy.outputs.set(name, po);
}

pub(super) async fn watch_displays(state: Rc<PortalState>) {
    let inotify = Rc::new(uapi::inotify_init1(c::IN_CLOEXEC).unwrap());
    if let Err(e) = uapi::inotify_add_watch(inotify.raw(), state.xrd.as_str(), c::IN_CREATE) {
        log::error!(
            "Cannot watch directory `{}`: {}",
            state.xrd,
            ErrorFmt(OsError::from(e))
        );
        return;
    }
    let rd = match std::fs::read_dir(&state.xrd) {
        Ok(rd) => rd,
        Err(e) => {
            log::error!("Cannot enumerate `{}`: {}", state.xrd, ErrorFmt(e));
            return;
        }
    };
    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                log::error!("Cannot enumerate `{}`: {}", state.xrd, ErrorFmt(e));
                return;
            }
        };
        if let Ok(s) = std::str::from_utf8(entry.file_name().as_bytes()) {
            maybe_add_display(&state, s).await;
        }
    }
    let mut buf = vec![0u8; 4096];
    loop {
        if let Err(e) = state.ring.readable(&inotify).await {
            log::error!("Cannot wait for `{}` to change: {}", state.xrd, ErrorFmt(e));
        }
        let events = match uapi::inotify_read(inotify.raw(), &mut buf[..]) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not read from inotify fd: {}", ErrorFmt(e));
                return;
            }
        };
        for event in events {
            if event.mask.contains(c::IN_CREATE) {
                if let Ok(s) = std::str::from_utf8(event.name().as_ustr().as_bytes()) {
                    maybe_add_display(&state, s).await;
                }
            }
        }
    }
}
