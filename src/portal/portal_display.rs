use {
    crate::{
        portal::{portal_render_ctx::PortalRenderCtx, PortalState},
        render::RenderContext,
        utils::{
            bitflags::BitflagsExt, clonecell::CloneCell, copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt, oserror::OsError,
        },
        video::drm::Drm,
        wire::{jay_output::LinearId, JayCompositor, WlOutput},
        wl_usr::{
            usr_ifs::{
                usr_jay_compositor::UsrJayCompositor,
                usr_jay_output::{UsrJayOutput, UsrJayOutputOwner},
                usr_jay_render_ctx::{UsrJayRenderCtx, UsrJayRenderCtxOwner},
                usr_wl_output::{UsrWlOutput, UsrWlOutputOwner},
                usr_wl_registry::{UsrWlRegistry, UsrWlRegistryOwner},
            },
            UsrCon, UsrConOwner,
        },
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        convert::Infallible,
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

pub struct PortalDisplay {
    id: u32,
    pub con: Rc<UsrCon>,
    state: Rc<PortalState>,
    registry: Rc<UsrWlRegistry>,
    pub jc: Rc<UsrJayCompositor>,
    pub outputs: CopyHashMap<u32, Rc<PortalOutput>>,
    pub render_ctx: CloneCell<Option<Rc<PortalRenderCtx>>>,
}

pub struct PortalOutput {
    linear_id: Cell<u32>,
    dpy: Rc<PortalDisplay>,
    wl: Rc<UsrWlOutput>,
    pub jay: Rc<UsrJayOutput>,
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
            Ok(s) => s.st_dev,
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
        if self.render_ctx.get().is_none() {
            let drm = Drm::open_existing(fd);
            let ctx = match RenderContext::from_drm_device(&drm) {
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
                dev_id,
                ctx: Rc::new(ctx),
            });
            self.render_ctx.set(Some(ctx.clone()));
            self.state.render_ctxs.set(dev_id, Rc::downgrade(&ctx));
        }
    }
}

impl UsrConOwner for PortalDisplay {
    fn killed(&self) {
        log::info!("Removing display {}", self.id);
        self.state.displays.remove(&self.id);
    }
}

impl UsrWlRegistryOwner for PortalDisplay {
    fn global(self: Rc<Self>, name: u32, interface: &str, version: u32) {
        // todo
    }
}

impl UsrJayOutputOwner for PortalOutput {
    fn linear_id(self: Rc<Self>, ev: &LinearId) {
        log::info!("Display: {}: New output {}", self.dpy.id, ev.linear_id);
        self.linear_id.set(ev.linear_id);
        self.dpy.outputs.set(ev.linear_id, self.clone());
    }

    fn destroyed(&self) {
        let id = self.linear_id.get();
        if id != 0 {
            log::info!("Display {}: Output {} removed", self.dpy.con.server_id, id);
            self.dpy.outputs.remove(&id);
        }
        self.dpy.con.remove_obj(self.wl.deref());
        self.dpy.con.remove_obj(self.jay.deref());
    }
}

impl UsrWlOutputOwner for PortalOutput {}

fn maybe_add_display(state: &Rc<PortalState>, name: &str) {
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
    let con = match UsrCon::new(&state.ring, &state.wheel, &state.eng, &path, num) {
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
    con.request_sync::<Infallible, _>(move || {
        finish_display_connect(dpy);
        Ok(())
    });
    log::info!("Connected to wayland display {num}: {name}");
}

fn finish_display_connect(dpy: Rc<PortalDisplayPrelude>) {
    let mut jc_opt = None;
    let mut outputs = vec![];
    for (interface, instances) in dpy.globals.borrow_mut().deref() {
        for &(name, version) in instances {
            if interface == JayCompositor.name() {
                let jc = Rc::new(UsrJayCompositor {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    owner: Default::default(),
                });
                dpy.con.add_object(jc.clone());
                dpy.registry.request_bind(name, version, jc.deref());
                jc_opt = Some(jc);
            } else if interface == WlOutput.name() {
                let wl = Rc::new(UsrWlOutput {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    owner: Default::default(),
                });
                dpy.con.add_object(wl.clone());
                dpy.registry.request_bind(name, version, wl.deref());
                outputs.push(wl);
            }
        }
    }
    let jc = match jc_opt {
        Some(jc) => jc,
        _ => {
            log::error!("Compositor did not advertise a JayCompositor");
            dpy.con.kill();
            return;
        }
    };
    let dpy = Rc::new(PortalDisplay {
        id: dpy.state.next_id.fetch_add(1),
        con: dpy.con.clone(),
        state: dpy.state.clone(),
        registry: dpy.registry.clone(),
        jc,
        outputs: Default::default(),
        render_ctx: Default::default(),
    });

    dpy.state.displays.set(dpy.id, dpy.clone());
    dpy.con.owner.set(Some(dpy.clone()));
    dpy.registry.owner.set(Some(dpy.clone()));

    let jrc = Rc::new(UsrJayRenderCtx {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
    });
    jrc.owner.set(Some(dpy.clone()));
    dpy.jc.request_get_render_context(&jrc);
    dpy.con.add_object(jrc);

    for output in outputs {
        let jo = Rc::new(UsrJayOutput {
            id: dpy.con.id(),
            con: dpy.con.clone(),
            owner: Default::default(),
        });
        dpy.con.add_object(jo.clone());
        dpy.jc.request_get_output(&jo, &output);
        let po = Rc::new(PortalOutput {
            linear_id: Cell::new(0),
            dpy: dpy.clone(),
            wl: output.clone(),
            jay: jo.clone(),
        });
        po.wl.owner.set(Some(po.clone()));
        po.jay.owner.set(Some(po.clone()));
    }
    log::info!("Display {} initialized", dpy.id);
}

pub(super) async fn watch_displays(state: Rc<PortalState>) {
    let inotify = Rc::new(uapi::inotify_init1(c::IN_CLOEXEC | c::IN_NONBLOCK).unwrap());
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
            maybe_add_display(&state, s);
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
                    maybe_add_display(&state, s);
                }
            }
        }
    }
}
