mod input;
mod monitor;
mod video;

use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, InputDevice, InputDeviceAccelProfile, InputDeviceCapability,
            InputDeviceGroupId, InputDeviceId, InputEvent, KeyState, TransformMatrix,
        },
        backends::metal::video::{
            MetalDrmDeviceData, MetalLeaseData, MetalRenderContext, PendingDrmDevice,
        },
        dbus::{DbusError, SignalHandler},
        drm_feedback::DrmFeedback,
        gfx_api::GfxError,
        ifs::wl_seat::tablet::{
            TabletId, TabletInit, TabletPadGroupInit, TabletPadId, TabletPadInit,
        },
        libinput::{
            consts::{
                AccelProfile, LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
                LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT, LIBINPUT_DEVICE_CAP_TABLET_PAD,
                LIBINPUT_DEVICE_CAP_TABLET_TOOL,
            },
            device::{LibInputDevice, RegisteredDevice},
            LibInput, LibInputAdapter, LibInputError,
        },
        logind::{LogindError, Session},
        state::State,
        udev::{Udev, UdevError, UdevMonitor},
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt,
            numcell::NumCell,
            oserror::OsError,
            smallmap::SmallMap,
            syncqueue::SyncQueue,
        },
        video::{
            drm::{DrmError, DRM_MODE_ATOMIC_ALLOW_MODESET},
            gbm::GbmError,
        },
    },
    bstr::ByteSlice,
    std::{
        any::Any,
        cell::{Cell, RefCell},
        error::Error,
        ffi::{CStr, CString},
        fmt::{Debug, Formatter},
        future::pending,
        rc::Rc,
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum MetalError {
    #[error("Could not connect to the dbus system socket")]
    DbusSystemSocket(#[source] DbusError),
    #[error("Could not retrieve the logind session")]
    LogindSession(#[source] LogindError),
    #[error("Could not take control of the logind session")]
    TakeControl(#[source] LogindError),
    #[error("Could not enumerate devices")]
    Enumerate(#[source] Box<Self>),
    #[error(transparent)]
    Udev(#[from] UdevError),
    #[error(transparent)]
    LibInput(#[from] LibInputError),
    #[error("Dupfd failed")]
    Dup(#[source] crate::utils::oserror::OsError),
    #[error("Could not create GBM device")]
    GbmDevice(#[source] GbmError),
    #[error("Could not update the drm properties")]
    UpdateProperties(#[source] DrmError),
    #[error("Could not create a render context")]
    CreateRenderContex(#[source] GfxError),
    #[error("Cannot initialize connector because no CRTC is available")]
    NoCrtcForConnector,
    #[error("Cannot initialize connector because no primary plane is available")]
    NoPrimaryPlaneForConnector,
    #[error("Cannot initialize connector because no mode is available")]
    NoModeForConnector,
    #[error("Could not allocate scanout buffer")]
    ScanoutBuffer(#[source] GbmError),
    #[error("addfb2 failed")]
    Framebuffer(#[source] DrmError),
    #[error("Could not import a framebuffer into the graphics API")]
    ImportFb(#[source] GfxError),
    #[error("Could not import a texture into the graphics API")]
    ImportTexture(#[source] GfxError),
    #[error("Could not import an image into the graphics API")]
    ImportImage(#[source] GfxError),
    #[error("Could not perform modeset")]
    Modeset(#[source] DrmError),
    #[error("Could not enable atomic modesetting")]
    AtomicModesetting(#[source] OsError),
    #[error("Could not inspect a plane")]
    CreatePlane(#[source] DrmError),
    #[error("Could not inspect a crtc")]
    CreateCrtc(#[source] DrmError),
    #[error("Could not inspect an encoder")]
    CreateEncoder(#[source] DrmError),
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("Could not create device-paused signal handler")]
    DevicePauseSignalHandler(#[source] DbusError),
    #[error("Could not create device-resumed signal handler")]
    DeviceResumeSignalHandler(#[source] DbusError),
    #[error("Device render context does not support required format {0}")]
    MissingDevFormat(&'static str),
    #[error("Render context does not support required format {0}")]
    MissingRenderFormat(&'static str),
    #[error("Device cannot scan out any buffers writable by its GFX API (format {0})")]
    MissingDevModifier(&'static str),
    #[error("Device GFX API cannot read any buffers writable by the render GFX API (format {0})")]
    MissingRenderModifier(&'static str),
    #[error("Could not render the frame")]
    RenderFrame(#[source] GfxError),
    #[error("Could not copy frame to output device")]
    CopyToOutput(#[source] GfxError),
    #[error("Could not perform atomic commit")]
    Commit(#[source] DrmError),
    #[error("Could not clear framebuffer")]
    Clear(#[source] GfxError),
}

pub struct MetalBackend {
    state: Rc<State>,
    udev: Rc<Udev>,
    monitor: Rc<UdevMonitor>,
    monitor_fd: Rc<OwnedFd>,
    libinput: Rc<LibInput>,
    libinput_fd: Rc<OwnedFd>,
    device_holder: Rc<DeviceHolder>,
    session: Session,
    pause_handler: Cell<Option<SignalHandler>>,
    resume_handler: Cell<Option<SignalHandler>>,
    ctx: CloneCell<Option<Rc<MetalRenderContext>>>,
    default_feedback: CloneCell<Option<Rc<DrmFeedback>>>,
}

impl Debug for MetalBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalBackend").finish_non_exhaustive()
    }
}

impl MetalBackend {
    async fn run(self: Rc<Self>) -> Result<(), MetalError> {
        let _monitor = self.state.eng.spawn(self.clone().monitor_devices());
        let _events = self.state.eng.spawn(self.clone().handle_libinput_events());
        if let Err(e) = self.enumerate_devices() {
            return Err(MetalError::Enumerate(Box::new(e)));
        }
        pending().await
    }
}

impl Backend for MetalBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>> {
        let slf = self.clone();
        self.state.eng.spawn(async move {
            slf.run().await?;
            Ok(())
        })
    }

    fn clear(&self) {
        self.pause_handler.take();
        self.resume_handler.take();
        self.ctx.take();
        self.device_holder.devices.clear();
        for dev in self.device_holder.input_devices.take() {
            if let Some(dev) = dev {
                dev.inputdev.take();
                dev.events.take();
                dev.cb.take();
            }
        }
        for dev in self.device_holder.drm_devices.lock().drain_values() {
            dev.futures.clear();
            for crtc in dev.dev.crtcs.values() {
                crtc.connector.take();
            }
            dev.dev.handle_events.handle_events.take();
            dev.dev.on_change.clear();
            let clear_lease = |lease: &mut MetalLeaseData| {
                lease.connectors.clear();
                lease.crtcs.clear();
                lease.planes.clear();
            };
            for mut lease in dev.dev.leases.lock().drain_values() {
                clear_lease(&mut lease);
            }
            for mut lease in dev.dev.leases_to_break.lock().drain_values() {
                clear_lease(&mut lease);
            }
            for connector in dev.connectors.lock().drain_values() {
                {
                    let d = &mut *connector.display.borrow_mut();
                    d.crtcs.clear();
                }
                connector.primary_plane.take();
                connector.cursor_plane.take();
                connector.crtc.take();
                connector.on_change.clear();
                connector.present_trigger.clear();
                connector.render_result.take();
                connector.active_framebuffer.take();
                connector.next_framebuffer.take();
            }
        }
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn switch_to(&self, vtnr: u32) {
        self.session.switch_to(vtnr, move |res| {
            if let Err(e) = res {
                log::error!("Could not switch to VT {}: {}", vtnr, ErrorFmt(e));
            }
        })
    }

    fn set_idle(&self, idle: bool) {
        let devices = self.device_holder.drm_devices.lock();
        for device in devices.values() {
            let mut change = device.dev.master.change();
            for connector in device.connectors.lock().values() {
                if let Some(crtc) = connector.crtc.get() {
                    if idle == crtc.active.value.get() {
                        crtc.active.value.set(!idle);
                        change.change_object(crtc.id, |c| {
                            c.change(crtc.active.id, (!idle) as _);
                        });
                    }
                }
            }
            if let Err(e) = change.commit(DRM_MODE_ATOMIC_ALLOW_MODESET, 0) {
                log::error!("Could not set monitors idle/not idle: {}", ErrorFmt(e));
                return;
            }
        }
        if idle {
            self.state.set_backend_idle(true);
        } else {
            for device in devices.values() {
                for connector in device.connectors.lock().values() {
                    connector.schedule_present();
                }
            }
        }
    }

    fn import_environment(&self) -> bool {
        true
    }

    fn supports_presentation_feedback(&self) -> bool {
        true
    }
}

fn dup_fd(fd: c::c_int) -> Result<Rc<OwnedFd>, MetalError> {
    match uapi::fcntl_dupfd_cloexec(fd, 0) {
        Ok(m) => Ok(Rc::new(m)),
        Err(e) => Err(MetalError::Dup(e.into())),
    }
}

pub async fn create(state: &Rc<State>) -> Result<Rc<MetalBackend>, MetalError> {
    let socket = match state.dbus.system().await {
        Ok(s) => s,
        Err(e) => return Err(MetalError::DbusSystemSocket(e)),
    };
    let session = match Session::get(&socket).await {
        Ok(s) => s,
        Err(e) => return Err(MetalError::LogindSession(e)),
    };
    if let Err(e) = session.take_control().await {
        return Err(MetalError::TakeControl(e));
    }
    let device_holder = Rc::new(DeviceHolder {
        devices: Default::default(),
        input_devices: Default::default(),
        drm_devices: Default::default(),
        pending_drm_devices: Default::default(),
        num_pending_devices: Default::default(),
    });
    let udev = Rc::new(Udev::new()?);
    let monitor = Rc::new(udev.create_monitor()?);
    monitor.add_match_subsystem_devtype(Some("input"), None)?;
    monitor.add_match_subsystem_devtype(Some("drm"), None)?;
    monitor.enable_receiving()?;
    let libinput = Rc::new(LibInput::new(device_holder.clone())?);
    let monitor_fd = dup_fd(monitor.fd())?;
    let libinput_fd = dup_fd(libinput.fd())?;
    let metal = Rc::new(MetalBackend {
        state: state.clone(),
        udev,
        monitor,
        monitor_fd,
        libinput,
        libinput_fd,
        device_holder,
        session,
        pause_handler: Default::default(),
        resume_handler: Default::default(),
        ctx: Default::default(),
        default_feedback: Default::default(),
    });
    metal.pause_handler.set(Some({
        let mtl = metal.clone();
        let sh = metal.session.on_pause(move |p| mtl.handle_device_pause(p));
        match sh {
            Ok(sh) => sh,
            Err(e) => return Err(MetalError::DevicePauseSignalHandler(e)),
        }
    }));
    metal.resume_handler.set(Some({
        let mtl = metal.clone();
        let sh = metal
            .session
            .on_resume(move |p| mtl.handle_device_resume(p));
        match sh {
            Ok(sh) => sh,
            Err(e) => return Err(MetalError::DeviceResumeSignalHandler(e)),
        }
    }));
    Ok(metal)
}

struct MetalInputDevice {
    state: Rc<State>,
    slot: usize,
    id: InputDeviceId,
    fully_initialized: Cell<bool>,
    devnum: c::dev_t,
    fd: CloneCell<Option<Rc<OwnedFd>>>,
    inputdev: CloneCell<Option<Rc<RegisteredDevice>>>,
    devnode: CString,
    _sysname: CString,
    syspath: CString,
    removed: Cell<bool>,
    events: SyncQueue<InputEvent>,
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
    name: CloneCell<Rc<String>>,
    transform_matrix: Cell<Option<TransformMatrix>>,
    tablet_id: Cell<Option<TabletId>>,
    tablet_pad_id: Cell<Option<TabletPadId>>,

    // state
    pressed_keys: SmallMap<u32, (), 5>,
    pressed_buttons: SmallMap<u32, (), 2>,

    // config
    desired: InputDeviceProperties,
    effective: InputDeviceProperties,
}

#[derive(Default)]
struct InputDeviceProperties {
    left_handed: Cell<Option<bool>>,
    accel_profile: Cell<Option<AccelProfile>>,
    accel_speed: Cell<Option<f64>>,
    tap_enabled: Cell<Option<bool>>,
    drag_enabled: Cell<Option<bool>>,
    drag_lock_enabled: Cell<Option<bool>>,
    natural_scrolling_enabled: Cell<Option<bool>>,
}

#[derive(Clone)]
enum MetalDevice {
    Input(Rc<MetalInputDevice>),
    Drm(Rc<MetalDrmDeviceData>),
}

unsafe impl UnsafeCellCloneSafe for MetalDevice {}

struct DeviceHolder {
    devices: CopyHashMap<c::dev_t, MetalDevice>,
    input_devices: RefCell<Vec<Option<Rc<MetalInputDevice>>>>,
    drm_devices: CopyHashMap<c::dev_t, Rc<MetalDrmDeviceData>>,
    pending_drm_devices: CopyHashMap<c::dev_t, PendingDrmDevice>,
    num_pending_devices: NumCell<u32>,
}

impl LibInputAdapter for DeviceHolder {
    fn open(&self, path: &CStr) -> Result<OwnedFd, LibInputError> {
        let stat = match uapi::stat(path) {
            Ok(s) => s,
            Err(e) => return Err(LibInputError::Stat(e.into())),
        };
        if let Some(MetalDevice::Input(d)) = self.devices.get(&stat.st_rdev) {
            if let Some(fd) = d.fd.get() {
                return uapi::fcntl_dupfd_cloexec(fd.raw(), 0)
                    .map_err(|e| LibInputError::DupFd(e.into()));
            }
        }
        Err(LibInputError::DeviceUnavailable)
    }
}

impl MetalInputDevice {
    fn apply_config(&self) {
        if self.inputdev.is_none() {
            return;
        }
        if let Some(lh) = self.desired.left_handed.get() {
            self.set_left_handed(lh);
        }
        if let Some(profile) = self.desired.accel_profile.get() {
            self.set_accel_profile_(profile);
        }
        if let Some(speed) = self.desired.accel_speed.get() {
            self.set_accel_speed(speed);
        }
        if let Some(enabled) = self.desired.tap_enabled.get() {
            self.set_tap_enabled(enabled);
        }
        if let Some(enabled) = self.desired.drag_enabled.get() {
            self.set_drag_enabled(enabled);
        }
        if let Some(enabled) = self.desired.drag_lock_enabled.get() {
            self.set_drag_lock_enabled(enabled);
        }
        if let Some(enabled) = self.desired.natural_scrolling_enabled.get() {
            self.set_natural_scrolling_enabled(enabled);
        }
        self.fetch_effective();
    }

    fn fetch_effective(&self) {
        let Some(dev) = self.inputdev.get() else {
            return;
        };
        let device = dev.device();
        if device.left_handed_available() {
            self.effective.left_handed.set(Some(device.left_handed()));
        }
        if device.accel_available() {
            self.effective
                .accel_profile
                .set(Some(device.accel_profile()));
            self.effective.accel_speed.set(Some(device.accel_speed()));
        }
        if device.tap_available() {
            self.effective.tap_enabled.set(Some(device.tap_enabled()));
            self.effective.drag_enabled.set(Some(device.drag_enabled()));
            self.effective
                .drag_lock_enabled
                .set(Some(device.drag_lock_enabled()));
        }
        if device.has_natural_scrolling() {
            self.effective
                .natural_scrolling_enabled
                .set(Some(device.natural_scrolling_enabled()));
        }
    }

    fn pre_pause(&self) {
        let time_usec = self.state.now_usec();
        for (key, _) in self.pressed_keys.take() {
            self.event(InputEvent::Key {
                time_usec,
                key,
                state: KeyState::Released,
            });
        }
        for (button, _) in self.pressed_buttons.take() {
            self.event(InputEvent::Button {
                time_usec,
                button,
                state: KeyState::Released,
            });
        }
    }

    fn set_accel_profile_(&self, profile: AccelProfile) {
        self.desired.accel_profile.set(Some(profile));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().accel_available() {
                dev.device().set_accel_profile(profile);
                self.effective
                    .accel_profile
                    .set(Some(dev.device().accel_profile()));
            }
        }
    }
}

impl InputDevice for MetalInputDevice {
    fn id(&self) -> InputDeviceId {
        self.id
    }

    fn removed(&self) -> bool {
        self.removed.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.cb.set(Some(cb));
    }

    fn grab(&self, _grab: bool) {
        // nothing
    }

    fn has_capability(&self, cap: InputDeviceCapability) -> bool {
        let li = cap.to_libinput();
        match self.inputdev.get() {
            Some(dev) => dev.device().has_cap(li),
            _ => false,
        }
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.desired.left_handed.set(Some(left_handed));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().left_handed_available() {
                dev.device().set_left_handed(left_handed);
                self.effective
                    .left_handed
                    .set(Some(dev.device().left_handed()));
            }
        }
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        let profile = match profile {
            InputDeviceAccelProfile::Flat => LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT,
            InputDeviceAccelProfile::Adaptive => LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
        };
        self.set_accel_profile_(profile);
    }

    fn set_accel_speed(&self, speed: f64) {
        self.desired.accel_speed.set(Some(speed));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().accel_available() {
                dev.device().set_accel_speed(speed);
                self.effective
                    .accel_speed
                    .set(Some(dev.device().accel_speed()));
            }
        }
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(Some(matrix));
    }

    fn name(&self) -> Rc<String> {
        self.name.get()
    }

    fn dev_t(&self) -> Option<c::dev_t> {
        Some(self.devnum)
    }

    fn set_tap_enabled(&self, enabled: bool) {
        self.desired.tap_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().tap_available() {
                dev.device().set_tap_enabled(enabled);
                self.effective
                    .tap_enabled
                    .set(Some(dev.device().tap_enabled()));
            }
        }
    }

    fn set_drag_enabled(&self, enabled: bool) {
        self.desired.drag_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().tap_available() {
                dev.device().set_drag_enabled(enabled);
                self.effective
                    .drag_enabled
                    .set(Some(dev.device().drag_enabled()));
            }
        }
    }

    fn set_drag_lock_enabled(&self, enabled: bool) {
        self.desired.drag_lock_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().tap_available() {
                dev.device().set_drag_lock_enabled(enabled);
                self.effective
                    .drag_lock_enabled
                    .set(Some(dev.device().drag_lock_enabled()));
            }
        }
    }

    fn set_natural_scrolling_enabled(&self, enabled: bool) {
        self.desired.natural_scrolling_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            if dev.device().has_natural_scrolling() {
                dev.device().set_natural_scrolling_enabled(enabled);
                self.effective
                    .natural_scrolling_enabled
                    .set(Some(dev.device().natural_scrolling_enabled()));
            }
        }
    }

    fn left_handed(&self) -> Option<bool> {
        self.effective.left_handed.get()
    }

    fn accel_profile(&self) -> Option<InputDeviceAccelProfile> {
        let p = self.effective.accel_profile.get()?;
        let p = match p {
            LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT => InputDeviceAccelProfile::Flat,
            LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE => InputDeviceAccelProfile::Adaptive,
            _ => return None,
        };
        Some(p)
    }

    fn accel_speed(&self) -> Option<f64> {
        self.effective.accel_speed.get()
    }

    fn transform_matrix(&self) -> Option<TransformMatrix> {
        self.transform_matrix.get()
    }

    fn tap_enabled(&self) -> Option<bool> {
        self.effective.tap_enabled.get()
    }

    fn drag_enabled(&self) -> Option<bool> {
        self.effective.drag_enabled.get()
    }

    fn drag_lock_enabled(&self) -> Option<bool> {
        self.effective.drag_lock_enabled.get()
    }

    fn natural_scrolling_enabled(&self) -> Option<bool> {
        self.effective.natural_scrolling_enabled.get()
    }

    fn tablet_info(&self) -> Option<Box<TabletInit>> {
        let dev = self.inputdev.get()?;
        let dev = dev.device();
        if !dev.has_cap(LIBINPUT_DEVICE_CAP_TABLET_TOOL) {
            return None;
        }
        let id = match self.tablet_id.get() {
            Some(id) => id,
            None => {
                let id = self.state.tablet_ids.next();
                self.tablet_id.set(Some(id));
                id
            }
        };
        Some(Box::new(TabletInit {
            id,
            group: self.get_device_group(&dev),
            name: dev.name(),
            pid: dev.product(),
            vid: dev.vendor(),
            path: self.syspath.as_bytes().as_bstr().to_string(),
        }))
    }

    fn tablet_pad_info(&self) -> Option<Box<TabletPadInit>> {
        let dev = self.inputdev.get()?;
        let dev = dev.device();
        if !dev.has_cap(LIBINPUT_DEVICE_CAP_TABLET_PAD) {
            return None;
        }
        let id = match self.tablet_pad_id.get() {
            Some(id) => id,
            None => {
                let id = self.state.tablet_pad_ids.next();
                self.tablet_pad_id.set(Some(id));
                id
            }
        };
        let buttons = dev.pad_num_buttons();
        let strips = dev.pad_num_strips();
        let rings = dev.pad_num_rings();
        let mut groups = vec![];
        for n in 0..dev.pad_num_mode_groups() {
            let Some(group) = dev.pad_mode_group(n) else {
                break;
            };
            groups.push(TabletPadGroupInit {
                buttons: (0..buttons).filter(|b| group.has_button(*b)).collect(),
                rings: (0..rings).filter(|b| group.has_ring(*b)).collect(),
                strips: (0..strips).filter(|b| group.has_strip(*b)).collect(),
                modes: group.num_modes(),
                mode: group.mode(),
            });
        }
        Some(Box::new(TabletPadInit {
            id,
            group: self.get_device_group(&dev),
            path: self.syspath.as_bytes().as_bstr().to_string(),
            buttons,
            strips,
            rings,
            groups,
        }))
    }
}

impl MetalInputDevice {
    fn event(&self, event: InputEvent) {
        self.events.push(event);
        if let Some(cb) = self.cb.get() {
            cb();
        }
    }

    fn get_device_group(&self, dev: &LibInputDevice) -> InputDeviceGroupId {
        let group = dev.device_group();
        let mut id = group.user_data();
        if id == 0 {
            id = self.state.input_device_group_ids.next().raw();
            group.set_user_data(id);
        }
        InputDeviceGroupId::from_raw(id)
    }
}
