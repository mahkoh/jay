mod input;
mod monitor;
mod video;

use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, InputDevice, InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId,
            InputEvent, KeyState, TransformMatrix,
        },
        backends::metal::video::{MetalDrmDeviceData, MetalRenderContext, PendingDrmDevice},
        dbus::{DbusError, SignalHandler},
        gfx_api::GfxError,
        libinput::{
            consts::{
                AccelProfile, LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
                LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT, LIBINPUT_DEVICE_CAP_GESTURE,
                LIBINPUT_DEVICE_CAP_KEYBOARD, LIBINPUT_DEVICE_CAP_POINTER,
                LIBINPUT_DEVICE_CAP_SWITCH, LIBINPUT_DEVICE_CAP_TABLET_PAD,
                LIBINPUT_DEVICE_CAP_TABLET_TOOL, LIBINPUT_DEVICE_CAP_TOUCH,
            },
            device::RegisteredDevice,
            LibInput, LibInputAdapter, LibInputError,
        },
        logind::{LogindError, Session},
        state::State,
        time::now_usec,
        udev::{Udev, UdevError, UdevMonitor},
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
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
        if !idle {
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
    slot: usize,
    id: InputDeviceId,
    _devnum: c::dev_t,
    fd: CloneCell<Option<Rc<OwnedFd>>>,
    inputdev: CloneCell<Option<Rc<RegisteredDevice>>>,
    devnode: CString,
    _sysname: CString,
    removed: Cell<bool>,
    events: SyncQueue<InputEvent>,
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
    name: CloneCell<Rc<String>>,
    natural_scrolling: Cell<bool>,

    // state
    pressed_keys: SmallMap<u32, (), 5>,
    pressed_buttons: SmallMap<u32, (), 2>,

    // config
    left_handed: Cell<Option<bool>>,
    accel_profile: Cell<Option<AccelProfile>>,
    accel_speed: Cell<Option<f64>>,
    transform_matrix: Cell<Option<TransformMatrix>>,
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
        let dev = match self.inputdev.get() {
            Some(dev) => dev,
            _ => return,
        };
        if let Some(lh) = self.left_handed.get() {
            dev.device().set_left_handed(lh);
        }
        if let Some(profile) = self.accel_profile.get() {
            dev.device().set_accel_profile(profile);
        }
        if let Some(speed) = self.accel_speed.get() {
            dev.device().set_accel_speed(speed);
        }
        if let Some(enabled) = self.tap_enabled.get() {
            dev.device().set_tap_enabled(enabled);
        }
        if let Some(enabled) = self.drag_enabled.get() {
            dev.device().set_drag_enabled(enabled);
        }
        if let Some(enabled) = self.drag_lock_enabled.get() {
            dev.device().set_drag_lock_enabled(enabled);
        }
        if let Some(enabled) = self.natural_scrolling_enabled.get() {
            self.do_set_natural_scrolling_enabled(&dev, enabled);
        }
    }

    fn pre_pause(&self) {
        let time_usec = now_usec();
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

    fn do_set_natural_scrolling_enabled(&self, dev: &RegisteredDevice, enabled: bool) {
        dev.device().set_natural_scrolling_enabled(enabled);
        self.natural_scrolling
            .set(dev.device().natural_scrolling_enabled());
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
        let li = match cap {
            InputDeviceCapability::Keyboard => LIBINPUT_DEVICE_CAP_KEYBOARD,
            InputDeviceCapability::Pointer => LIBINPUT_DEVICE_CAP_POINTER,
            InputDeviceCapability::Touch => LIBINPUT_DEVICE_CAP_TOUCH,
            InputDeviceCapability::TabletTool => LIBINPUT_DEVICE_CAP_TABLET_TOOL,
            InputDeviceCapability::TabletPad => LIBINPUT_DEVICE_CAP_TABLET_PAD,
            InputDeviceCapability::Gesture => LIBINPUT_DEVICE_CAP_GESTURE,
            InputDeviceCapability::Switch => LIBINPUT_DEVICE_CAP_SWITCH,
        };
        match self.inputdev.get() {
            Some(dev) => dev.device().has_cap(li),
            _ => false,
        }
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.left_handed.set(Some(left_handed));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_left_handed(left_handed);
        }
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        let profile = match profile {
            InputDeviceAccelProfile::Flat => LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT,
            InputDeviceAccelProfile::Adaptive => LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
        };
        self.accel_profile.set(Some(profile));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_accel_profile(profile);
        }
    }

    fn set_accel_speed(&self, speed: f64) {
        self.accel_speed.set(Some(speed));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_accel_speed(speed);
        }
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(Some(matrix));
    }

    fn name(&self) -> Rc<String> {
        self.name.get()
    }

    fn set_tap_enabled(&self, enabled: bool) {
        self.tap_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_tap_enabled(enabled);
        }
    }

    fn set_drag_enabled(&self, enabled: bool) {
        self.drag_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_drag_enabled(enabled);
        }
    }

    fn set_drag_lock_enabled(&self, enabled: bool) {
        self.drag_lock_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            dev.device().set_drag_lock_enabled(enabled);
        }
    }

    fn set_natural_scrolling_enabled(&self, enabled: bool) {
        self.natural_scrolling_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get() {
            self.do_set_natural_scrolling_enabled(&dev, enabled);
        }
    }
}

impl MetalInputDevice {
    fn event(&self, event: InputEvent) {
        self.events.push(event);
        if let Some(cb) = self.cb.get() {
            cb();
        }
    }
}
