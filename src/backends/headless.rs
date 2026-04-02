use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{Backend, BackendDrmDevice, BackendEvent, DrmDeviceId, DrmEvent},
        backends::headless::HeadlessBackendError::{
            CreateDrm, GetDrmNodes, MonitorFdFailed, MonitorFdReadable, NoDrmNodes, OpenDrmNode,
        },
        gfx_api::{GfxApi, GfxContext},
        io_uring::IoUringError,
        state::State,
        udev::{Udev, UdevDevice, UdevError, UdevMonitor},
        utils::{
            bitflags::BitflagsExt,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt,
            on_change::OnChange,
            oserror::{OsError, OsErrorExt2},
        },
        video::drm::{Drm, DrmError, DrmVersion, NodeType, get_drm_nodes_from_dev},
    },
    HeadlessBackendError::{
        AddUdevSubsystemMatch, CreateUdev, CreateUdevEnumerator, CreateUdevMonitor,
        DupUdevMonitorFd, EnableUdevReceiving, GetUdevEntry, ScanUdevDevices,
    },
    std::{cell::Cell, error::Error, rc::Rc},
    thiserror::Error,
    uapi::{
        AsUstr, OwnedFd,
        c::{self, dev_t},
        major, minor,
    },
};

#[derive(Debug, Error)]
pub enum HeadlessBackendError {
    #[error("Could not create a udev instance")]
    CreateUdev(#[source] UdevError),
    #[error("Could not create a udev monitor")]
    CreateUdevMonitor(#[source] UdevError),
    #[error("Could not add a udev subsystem match")]
    AddUdevSubsystemMatch(#[source] UdevError),
    #[error("Could not enable udev receiving")]
    EnableUdevReceiving(#[source] UdevError),
    #[error("Could not dup udev monitor fd")]
    DupUdevMonitorFd(#[source] OsError),
    #[error("Could not create a udev enumerator")]
    CreateUdevEnumerator(#[source] UdevError),
    #[error("Could not scan udev device")]
    ScanUdevDevices(#[source] UdevError),
    #[error("Could not get udev entry")]
    GetUdevEntry(#[source] UdevError),
    #[error("Could not determine DRM nodes of device")]
    GetDrmNodes(#[source] OsError),
    #[error("Device has no DRM nodes")]
    NoDrmNodes,
    #[error("Could not open DRM node")]
    OpenDrmNode(#[source] OsError),
    #[error("Could not create Drm object")]
    CreateDrm(#[source] DrmError),
    #[error("Could not wait for monitor FD to become readable")]
    MonitorFdReadable(#[source] IoUringError),
    #[error("The monitor FD failed")]
    MonitorFdFailed,
}

pub struct HeadlessBackend {
    state: Rc<State>,
    udev: Rc<Udev>,
    monitor: Rc<UdevMonitor>,
    monitor_fd: Rc<OwnedFd>,
    devs: CopyHashMap<dev_t, Rc<HeadlessDrmDevice>>,
    render_device: Cell<Option<DrmDeviceId>>,
}

struct HeadlessDrmDevice {
    backend: Rc<HeadlessBackend>,
    id: DrmDeviceId,
    dev: dev_t,
    api: Cell<GfxApi>,
    drm: Drm,
    ctx: CloneCell<Option<Rc<dyn GfxContext>>>,
    events: OnChange<DrmEvent>,
}

const DRM: &[u8] = b"drm";

pub async fn create(state: &Rc<State>) -> Result<Rc<HeadlessBackend>, HeadlessBackendError> {
    let udev = Rc::new(Udev::new().map_err(CreateUdev)?);
    let monitor = Rc::new(udev.create_monitor().map_err(CreateUdevMonitor)?);
    monitor
        .add_match_subsystem_devtype(Some(DRM), None)
        .map_err(AddUdevSubsystemMatch)?;
    monitor.enable_receiving().map_err(EnableUdevReceiving)?;
    let monitor_fd = uapi::fcntl_dupfd_cloexec(monitor.fd(), 0)
        .map(Rc::new)
        .map_os_err(DupUdevMonitorFd)?;
    Ok(Rc::new(HeadlessBackend {
        state: state.clone(),
        udev,
        monitor,
        monitor_fd,
        devs: Default::default(),
        render_device: Default::default(),
    }))
}

impl Backend for HeadlessBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>> {
        let slf = self.clone();
        self.state.eng.spawn("headless backend", async move {
            slf.run().await?;
            Ok(())
        })
    }

    fn clear(&self) {
        for dev in self.devs.lock().drain_values() {
            dev.ctx.take();
            dev.events.clear();
        }
    }
}

fn is_primary_node(n: &[u8]) -> bool {
    match n.strip_prefix(b"card") {
        Some(r) => r.iter().copied().all(|c| matches!(c, b'0'..=b'9')),
        _ => false,
    }
}

impl HeadlessBackend {
    async fn run(self: Rc<Self>) -> Result<(), HeadlessBackendError> {
        if let Some(acceptor) = self.state.acceptor.get() {
            println!("WAYLAND_DISPLAY={}", acceptor.socket_name());
        }
        let mut enumerate = self.udev.create_enumerate().map_err(CreateUdevEnumerator)?;
        enumerate
            .add_match_subsystem(DRM)
            .map_err(AddUdevSubsystemMatch)?;
        enumerate.scan_devices().map_err(ScanUdevDevices)?;
        let mut entry_opt = enumerate.get_list_entry().map_err(GetUdevEntry)?;
        while let Some(entry) = entry_opt.take() {
            if let Ok(dev) = self.udev.create_device_from_syspath(entry.name()) {
                self.handle_device_add(dev);
            }
            entry_opt = entry.next();
        }
        self.state
            .backend_events
            .push(BackendEvent::DevicesEnumerated);
        loop {
            let res = self
                .state
                .ring
                .readable(&self.monitor_fd)
                .await
                .map_err(MonitorFdReadable)?;
            if res.intersects(c::POLLERR | c::POLLHUP) {
                return Err(MonitorFdFailed);
            }
            while let Some(dev) = self.monitor.receive_device() {
                if let Some(action) = dev.action()
                    && action.as_ustr() == "add"
                {
                    self.handle_device_add(dev);
                }
            }
        }
    }

    fn handle_device_add(self: &Rc<Self>, dev: UdevDevice) {
        let num = dev.devnum();
        if let Err(e) = self.handle_device_add_(dev) {
            log::error!(
                "Could not add device {}:{}: {}",
                major(num),
                minor(num),
                ErrorFmt(e),
            );
        }
    }

    fn handle_device_add_(self: &Rc<Self>, dev: UdevDevice) -> Result<(), HeadlessBackendError> {
        let Some(subsystem) = dev.subsystem() else {
            return Ok(());
        };
        if subsystem.as_ustr() != DRM {
            return Ok(());
        }
        let Some(sysname) = dev.sysname() else {
            return Ok(());
        };
        if !is_primary_node(sysname.to_bytes()) {
            return Ok(());
        }
        let devnum = dev.devnum();
        if self.devs.contains(&devnum) {
            return Ok(());
        }
        let nodes = get_drm_nodes_from_dev(major(devnum), minor(devnum)).map_err(GetDrmNodes)?;
        let node = nodes
            .get(&NodeType::Render)
            .or_else(|| nodes.get(&NodeType::Primary))
            .ok_or(NoDrmNodes)?;
        let fd = uapi::open(&**node, c::O_RDWR | c::O_CLOEXEC, 0).map_os_err(OpenDrmNode)?;
        let drm = Drm::open_existing(Rc::new(fd)).map_err(CreateDrm)?;
        let dev = Rc::new(HeadlessDrmDevice {
            backend: self.clone(),
            id: self.state.drm_dev_ids.next(),
            dev: devnum,
            api: Cell::new(self.state.default_gfx_api.get()),
            drm,
            ctx: Default::default(),
            events: Default::default(),
        });
        self.devs.set(devnum, dev.clone());
        self.state
            .backend_events
            .push(BackendEvent::NewDrmDevice(dev));
        Ok(())
    }
}

impl HeadlessDrmDevice {
    fn create_ctx(&self, api: GfxApi) -> Option<Rc<dyn GfxContext>> {
        match self.backend.state.create_gfx_context(&self.drm, Some(api)) {
            Ok(c) => Some(c),
            Err(e) => {
                log::error!("Could not create GFX context: {}", ErrorFmt(e));
                None
            }
        }
    }
}

impl BackendDrmDevice for HeadlessDrmDevice {
    fn id(&self) -> DrmDeviceId {
        self.id
    }

    fn event(&self) -> Option<DrmEvent> {
        self.events.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.events.on_change.set(Some(cb));
    }

    fn dev_t(&self) -> dev_t {
        self.dev
    }

    fn make_render_device(&self) {
        if self.is_render_device() {
            return;
        }
        let ctx = match self.ctx.get() {
            Some(ctx) => ctx,
            _ => {
                let Some(ctx) = self.create_ctx(self.gtx_api()) else {
                    return;
                };
                self.ctx.set(Some(ctx.clone()));
                ctx
            }
        };
        self.backend.render_device.set(Some(self.id));
        self.backend.state.set_render_ctx(Some(ctx));
    }

    fn set_gfx_api(&self, api: GfxApi) {
        if self.api.get() == api {
            return;
        }
        if self.ctx.is_none() {
            self.api.set(api);
            return;
        }
        let Some(ctx) = self.create_ctx(api) else {
            return;
        };
        self.ctx.set(Some(ctx.clone()));
        self.api.set(api);
        self.events.send_event(DrmEvent::GfxApiChanged);
        if self.is_render_device() {
            self.backend.state.set_render_ctx(Some(ctx));
        }
    }

    fn gtx_api(&self) -> GfxApi {
        self.api.get()
    }

    fn version(&self) -> Result<DrmVersion, DrmError> {
        self.drm.version()
    }

    fn set_direct_scanout_enabled(&self, enabled: bool) {
        let _ = enabled;
    }

    fn is_render_device(&self) -> bool {
        self.backend.render_device.get() == Some(self.id)
    }
}
