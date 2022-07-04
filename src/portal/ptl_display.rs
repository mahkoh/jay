use {
    crate::{
        portal::{
            ptl_render_ctx::PortalRenderCtx, ptl_screencast::SelectedScreencast, PortalState,
        },
        render::RenderContext,
        utils::{
            bitflags::BitflagsExt, clonecell::CloneCell, copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt, oserror::OsError,
        },
        video::{drm::Drm},
        wire::{
            jay_output::LinearId, jay_workspace, JayCompositor, JayOutputId, JayWorkspaceId,
            WlOutput,
        },
        wl_usr::{
            usr_ifs::{
                usr_jay_compositor::UsrJayCompositor,
                usr_jay_output::{UsrJayOutput, UsrJayOutputOwner},
                usr_jay_render_ctx::{UsrJayRenderCtx, UsrJayRenderCtxOwner},
                usr_jay_workspace::{UsrJayWorkspace, UsrJayWorkspaceOwner},
                usr_jay_workspace_watcher::{UsrJayWorkspaceWatcher, UsrJayWorkspaceWatcherOwner},
                usr_linux_dmabuf::UsrLinuxDmabuf,
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
use crate::wire::{WlCompositor, ZwlrLayerShellV1, ZwpLinuxDmabufV1};
use crate::wl_usr::usr_ifs::usr_wl_compositor::UsrWlCompositor;
use crate::wl_usr::usr_ifs::usr_wlr_layer_shell::UsrWlrLayerShell;

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
    workspace_watcher: Rc<UsrJayWorkspaceWatcher>,
    pub dmabuf: CloneCell<Option<Rc<UsrLinuxDmabuf>>>,

    pub jc: Rc<UsrJayCompositor>,
    pub ls: Rc<UsrWlrLayerShell>,
    pub comp: Rc<UsrWlCompositor>,
    pub render_ctx: CloneCell<Option<Rc<PortalRenderCtx>>>,

    pub outputs: CopyHashMap<JayOutputId, Rc<PortalOutput>>,
    pub outputs_by_linear_id: CopyHashMap<u32, Rc<PortalOutput>>,

    pub workspaces: CopyHashMap<JayWorkspaceId, Rc<PortalWorkspace>>,
    pub workspaces_by_linear_id: CopyHashMap<u32, Rc<PortalWorkspace>>,

    pub screencasts: CopyHashMap<u32, Rc<SelectedScreencast>>,
}

pub struct PortalWorkspace {
    linear_id: Cell<u32>,
    dpy: Rc<PortalDisplay>,
    ws: Rc<UsrJayWorkspace>,
    name: RefCell<String>,
    output: CloneCell<Option<Rc<PortalOutput>>>,
}

pub struct PortalOutput {
    pub linear_id: Cell<u32>,
    dpy: Rc<PortalDisplay>,
    pub wl: Rc<UsrWlOutput>,
    workspaces: CopyHashMap<u32, Rc<PortalWorkspace>>,
    pub jay: Rc<UsrJayOutput>,
}

impl PortalWorkspace {
    fn detach_from_output(&self) {
        if let Some(output) = self.output.take() {
            output.workspaces.remove(&self.linear_id.get());
        }
    }
}

impl UsrJayWorkspaceOwner for PortalWorkspace {
    fn linear_id(self: Rc<Self>, ev: &jay_workspace::LinearId) {
        self.linear_id.set(ev.linear_id);
        self.dpy
            .workspaces_by_linear_id
            .set(ev.linear_id, self.clone());
    }

    fn name(&self, ev: &jay_workspace::Name) {
        log::info!("New workspace {}", ev.name);
        *self.name.borrow_mut() = ev.name.to_string();
    }

    fn destroyed(&self) {
        log::info!("Workspace {} removed", self.name.borrow_mut());
        self.detach_from_output();
        self.dpy.workspaces.remove(&self.ws.id);
        self.dpy
            .workspaces_by_linear_id
            .remove(&self.linear_id.get());
        self.dpy.con.remove_obj(self.ws.deref());
    }

    fn output(self: Rc<Self>, ev: &jay_workspace::Output) {
        self.detach_from_output();
        if let Some(output) = self.dpy.outputs_by_linear_id.get(&ev.output_linear_id) {
            output.workspaces.set(self.linear_id.get(), self.clone());
            self.output.set(Some(output));
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

impl UsrJayWorkspaceWatcherOwner for PortalDisplay {
    fn new(self: Rc<Self>, ev: Rc<UsrJayWorkspace>) {
        let owner = Rc::new(PortalWorkspace {
            linear_id: Cell::new(0),
            dpy: self.clone(),
            ws: ev.clone(),
            name: RefCell::new("".to_string()),
            output: Default::default(),
        });
        ev.owner.set(Some(owner.clone()));
        self.workspaces.set(ev.id, owner);
        self.con.add_object(ev);
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
        if interface == WlOutput.name() {
            add_output(&self, name, version);
        } else if interface == ZwpLinuxDmabufV1.name() {
            let ls = Rc::new(UsrLinuxDmabuf {
                id: self.con.id(),
                con: self.con.clone(),
                owner: Default::default(),
            });
            self.con.add_object(ls.clone());
            self.registry.request_bind(name, version, ls.deref());
            self.dmabuf.set(Some(ls));
        }
    }
}

impl UsrJayOutputOwner for PortalOutput {
    fn linear_id(self: Rc<Self>, ev: &LinearId) {
        log::info!(
            "Display: {}: New output {}",
            self.dpy.con.server_id,
            ev.linear_id
        );
        self.linear_id.set(ev.linear_id);
        self.dpy
            .outputs_by_linear_id
            .set(ev.linear_id, self.clone());
    }

    fn destroyed(&self) {
        log::info!(
            "Display {}: Output {} removed",
            self.dpy.con.server_id,
            self.linear_id.get()
        );
        self.dpy.outputs.remove(&self.jay.id);
        self.dpy.outputs_by_linear_id.remove(&self.linear_id.get());
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
    let mut ls_opt = None;
    let mut comp_opt = None;
    let mut dmabuf_opt = None;
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
            } else if interface == ZwlrLayerShellV1.name() {
                let ls = Rc::new(UsrWlrLayerShell {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                ls_opt = Some(ls);
            } else if interface == WlCompositor.name() {
                let ls = Rc::new(UsrWlCompositor {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                comp_opt = Some(ls);
            } else if interface == ZwpLinuxDmabufV1.name() {
                let ls = Rc::new(UsrLinuxDmabuf {
                    id: dpy.con.id(),
                    con: dpy.con.clone(),
                    owner: Default::default(),
                });
                dpy.con.add_object(ls.clone());
                dpy.registry.request_bind(name, version, ls.deref());
                dmabuf_opt = Some(ls);
            } else if interface == WlOutput.name() {
                outputs.push((name, version));
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
        }
    }
    let jc = get!(jc_opt, JayCompositor);
    let ls = get!(ls_opt, ZwlrLayerShellV1);
    let comp = get!(comp_opt, WlCompositor);

    let workspace_watcher = Rc::new(UsrJayWorkspaceWatcher {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
    });
    dpy.con.add_object(workspace_watcher.clone());
    jc.request_watch_workspaces(&workspace_watcher);

    let dpy = Rc::new(PortalDisplay {
        id: dpy.state.id(),
        con: dpy.con.clone(),
        state: dpy.state.clone(),
        registry: dpy.registry.clone(),
        workspace_watcher,
        dmabuf: CloneCell::new(dmabuf_opt),
        jc,
        outputs: Default::default(),
        render_ctx: Default::default(),
        workspaces: Default::default(),
        outputs_by_linear_id: Default::default(),
        workspaces_by_linear_id: Default::default(),
        screencasts: Default::default(),
        ls,
        comp
    });

    dpy.state.displays.set(dpy.id, dpy.clone());
    dpy.con.owner.set(Some(dpy.clone()));
    dpy.registry.owner.set(Some(dpy.clone()));
    dpy.workspace_watcher.owner.set(Some(dpy.clone()));

    let jrc = Rc::new(UsrJayRenderCtx {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
    });
    jrc.owner.set(Some(dpy.clone()));
    dpy.jc.request_get_render_context(&jrc);
    dpy.con.add_object(jrc);

    for (name, version) in outputs {
        add_output(&dpy, name, version);
    }
    log::info!("Display {} initialized", dpy.id);
}

fn add_output(dpy: &Rc<PortalDisplay>, name: u32, version: u32) {
    let wl = Rc::new(UsrWlOutput {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
    });
    dpy.con.add_object(wl.clone());
    dpy.registry.request_bind(name, version, wl.deref());
    let jo = Rc::new(UsrJayOutput {
        id: dpy.con.id(),
        con: dpy.con.clone(),
        owner: Default::default(),
    });
    dpy.con.add_object(jo.clone());
    dpy.jc.request_get_output(&jo, &wl);
    let po = Rc::new(PortalOutput {
        linear_id: Cell::new(0),
        dpy: dpy.clone(),
        wl: wl.clone(),
        workspaces: Default::default(),
        jay: jo.clone(),
    });
    po.wl.owner.set(Some(po.clone()));
    po.jay.owner.set(Some(po.clone()));
    dpy.outputs.set(jo.id, po);
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
