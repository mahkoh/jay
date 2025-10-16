mod input;
mod monitor;
mod present;
mod transaction;
mod video;

use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, ButtonState, InputDevice, InputDeviceAccelProfile, InputDeviceCapability,
            InputDeviceClickMethod, InputDeviceGroupId, InputDeviceId, InputEvent, KeyState, Leds,
            TransformMatrix, transaction::BackendConnectorTransactionError,
        },
        backends::metal::video::{
            MetalDrmDeviceData, MetalLeaseData, MetalRenderContext, PendingDrmDevice,
            PersistentDisplayData,
        },
        dbus::{DbusError, SignalHandler},
        drm_feedback::DrmFeedback,
        format::Format,
        gfx_api::{GfxError, SyncFile},
        ifs::{
            wl_output::OutputId,
            wl_seat::tablet::{
                TabletId, TabletInit, TabletPadGroupInit, TabletPadId, TabletPadInit,
            },
        },
        libinput::{
            LibInput, LibInputAdapter, LibInputError,
            consts::{
                AccelProfile, ConfigClickMethod, LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
                LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT, LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS,
                LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER, LIBINPUT_CONFIG_CLICK_METHOD_NONE,
                LIBINPUT_DEVICE_CAP_TABLET_PAD, LIBINPUT_DEVICE_CAP_TABLET_TOOL, Led,
            },
            device::{LibInputDevice, RegisteredDevice},
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
        video::{Modifier, drm::DrmError, gbm::GbmError},
    },
    bstr::ByteSlice,
    indexmap::IndexSet,
    std::{
        cell::{Cell, RefCell},
        error::Error,
        ffi::{CStr, CString},
        fmt::{Debug, Display, Formatter},
        future::pending,
        rc::Rc,
    },
    thiserror::Error,
    uapi::{OwnedFd, c},
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
    #[error("Could not perform modeset")]
    Modeset(#[source] BackendConnectorTransactionError),
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
    #[error("Could not render the frame")]
    RenderFrame(#[source] GfxError),
    #[error("Could not copy frame to output device")]
    CopyToOutput(#[source] GfxError),
    #[error("Could not perform atomic commit")]
    Commit(#[source] DrmError),
    #[error("The present configuration is out of date")]
    OutOfDate,
    #[error("Could not add connector to transaction")]
    AddToTransaction(#[source] BackendConnectorTransactionError),
    #[error("Could not calculate DRM state")]
    CalculateDrmState(#[source] BackendConnectorTransactionError),
    #[error("Could not calculate DRM change set")]
    CalculateDrmChange(#[source] BackendConnectorTransactionError),
    #[error("Could not create plane buffer")]
    AllocateScanoutBuffer(#[source] Box<ScanoutBufferError>),
}

#[derive(Debug, Error)]
pub enum ScanoutBufferErrorKind {
    #[error("Scanout device: The format is not supported")]
    SodUnsupportedFormat,
    #[error(
        "Scanout device: The intersection of the modifiers supported by the plane and modifiers writable by the gfx API is empty"
    )]
    SodNoWritableModifier,
    #[error("Scanout device: Buffer allocation failed")]
    SodBufferAllocation(#[source] GbmError),
    #[error("Scanout device: addfb2 failed")]
    SodAddfb2(#[source] DrmError),
    #[error("Scanout device: Could not import SCANOUT buffer into the gfx API")]
    SodImportSodImage(#[source] GfxError),
    #[error("Scanout device: Could not turn imported SCANOUT buffer into gfx API FB")]
    SodImportFb(#[source] GfxError),
    #[error("Scanout device: Could not clear SCANOUT buffer")]
    SodClear(#[source] GfxError),
    #[error("Scanout device: Could not turn imported SCANOUT buffer into gfx API texture")]
    SodImportSodTexture(#[source] GfxError),
    #[error("Render device: The format is not supported")]
    RenderUnsupportedFormat,
    #[error(
        "Render device: The intersection of the modifiers readable by the scanout device and modifiers writable by the gfx API is empty"
    )]
    RenderNoWritableModifier,
    #[error("Render device: Buffer allocation failed")]
    RenderBufferAllocation(#[source] GbmError),
    #[error("Render device: Could not import RENDER buffer into the gfx API")]
    RenderImportImage(#[source] GfxError),
    #[error("Render device: Could not turn imported RENDER buffer into gfx API FB")]
    RenderImportFb(#[source] GfxError),
    #[error("Render device: Could not clear RENDER buffer")]
    RenderClear(#[source] GfxError),
    #[error("Render device: Could not turn imported RENDER buffer into gfx API texture")]
    RenderImportRenderTexture(#[source] GfxError),
    #[error("Scanout device: Could not import RENDER buffer into the gfx API")]
    SodImportRenderImage(#[source] GfxError),
    #[error("Scanout device: Could not turn imported RENDER buffer into gfx API texture")]
    SodImportRenderTexture(#[source] GfxError),
}

#[derive(Debug)]
pub struct ScanoutBufferError {
    dev: String,
    format: &'static Format,
    plane_modifiers: IndexSet<Modifier>,
    width: i32,
    height: i32,
    cursor: bool,
    dev_gfx_write_modifiers: Option<IndexSet<Modifier>>,
    dev_gfx_read_modifiers: Option<IndexSet<Modifier>>,
    dev_modifiers_possible: Option<IndexSet<Modifier>>,
    dev_usage: Option<u32>,
    dev_modifier: Option<Modifier>,
    render_name: Option<String>,
    render_gfx_write_modifiers: Option<IndexSet<Modifier>>,
    render_modifiers_possible: Option<IndexSet<Modifier>>,
    render_usage: Option<u32>,
    render_modifier: Option<Modifier>,
    kind: ScanoutBufferErrorKind,
}

impl Display for ScanoutBufferError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        writeln!(f, "scanout device: {}", self.dev)?;
        writeln!(f, "format: {}", self.format.name)?;
        writeln!(f, "plane modifiers: {:x?}", self.plane_modifiers)?;
        writeln!(f, "size: {}x{}", self.width, self.height)?;
        writeln!(f, "cursor: {}", self.cursor)?;
        if let Some(v) = &self.dev_gfx_write_modifiers {
            writeln!(f, "scanout gfx writable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dev_modifiers_possible {
            writeln!(f, "scanout dev possible modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dev_usage {
            writeln!(f, "scanout dev gbm usage: {:x}", v)?;
        }
        if let Some(v) = &self.dev_modifier {
            writeln!(f, "scanout dev modifier: {:x}", v)?;
        }
        if let Some(v) = &self.render_name {
            writeln!(f, "render device: {}", v)?;
        }
        if let Some(v) = &self.render_gfx_write_modifiers {
            writeln!(f, "render gfx writable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.dev_gfx_read_modifiers {
            writeln!(f, "scanout gfx readable modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.render_modifiers_possible {
            writeln!(f, "render dev possible modifiers: {:x?}", v)?;
        }
        if let Some(v) = &self.render_usage {
            writeln!(f, "render dev gbm usage: {:x}", v)?;
        }
        if let Some(v) = &self.render_modifier {
            writeln!(f, "render dev modifier: {:x}", v)?;
        }
        Ok(())
    }
}

impl Error for ScanoutBufferError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
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
    signaled_sync_file: CloneCell<Option<SyncFile>>,
    default_feedback: CloneCell<Option<Rc<DrmFeedback>>>,
    persistent_display_data: CopyHashMap<Rc<OutputId>, Rc<PersistentDisplayData>>,
}

impl Debug for MetalBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalBackend").finish_non_exhaustive()
    }
}

impl MetalBackend {
    async fn run(self: Rc<Self>) -> Result<(), MetalError> {
        let _monitor = self
            .state
            .eng
            .spawn("monitor devices", self.clone().monitor_devices());
        let _events = self.state.eng.spawn(
            "handle libinput events",
            self.clone().handle_libinput_events(),
        );
        if let Err(e) = self.enumerate_devices() {
            return Err(MetalError::Enumerate(Box::new(e)));
        }
        pending().await
    }
}

impl Backend for MetalBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>> {
        let slf = self.clone();
        self.state.eng.spawn("metal backend", async move {
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
                crtc.pending_flip.take();
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
                connector.active_framebuffer.take();
                connector.next_framebuffer.take();
            }
        }
    }

    fn switch_to(&self, vtnr: u32) {
        self.session.switch_to(vtnr, move |res| {
            if let Err(e) = res {
                log::error!("Could not switch to VT {}: {}", vtnr, ErrorFmt(e));
            }
        })
    }

    fn import_environment(&self) -> bool {
        true
    }

    fn supports_presentation_feedback(&self) -> bool {
        true
    }

    fn get_input_fds(&self) -> Vec<Rc<OwnedFd>> {
        let mut res = vec![];
        for dev in &*self.device_holder.input_devices.borrow() {
            if let Some(dev) = dev
                && let Some(fd) = dev.fd.get()
            {
                res.push(fd);
            }
        }
        res
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
        signaled_sync_file: Default::default(),
        default_feedback: Default::default(),
        persistent_display_data: Default::default(),
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
    calibration_matrix: Cell<Option<[[f32; 3]; 2]>>,
    click_method: Cell<Option<ConfigClickMethod>>,
    middle_button_emulation_enabled: Cell<Option<bool>>,
    enabled_leds: Cell<Option<Led>>,
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
        if let Some(MetalDevice::Input(d)) = self.devices.get(&stat.st_rdev)
            && let Some(fd) = d.fd.get()
        {
            return uapi::fcntl_dupfd_cloexec(fd.raw(), 0)
                .map_err(|e| LibInputError::DupFd(e.into()));
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
        if let Some(lh) = self.desired.calibration_matrix.get() {
            self.set_calibration_matrix(lh);
        }
        if let Some(method) = self.desired.click_method.get() {
            self.set_click_method_(method);
        }
        if let Some(enabled) = self.desired.middle_button_emulation_enabled.get() {
            self.set_middle_button_emulation_enabled(enabled);
        }
        if let Some(led) = self.desired.enabled_leds.get() {
            self.set_enabled_leds_(led);
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
        if device.has_calibration_matrix() {
            self.effective
                .calibration_matrix
                .set(Some(device.get_calibration_matrix()));
        }
        if device.has_click_methods() {
            self.effective.click_method.set(Some(device.click_method()));
        }
        if device.middle_button_emulation_available() {
            self.effective
                .middle_button_emulation_enabled
                .set(Some(device.middle_button_emulation_enabled()));
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
                state: ButtonState::Released,
            });
        }
    }

    fn set_accel_profile_(&self, profile: AccelProfile) {
        self.desired.accel_profile.set(Some(profile));
        if let Some(dev) = self.inputdev.get()
            && dev.device().accel_available()
        {
            dev.device().set_accel_profile(profile);
            self.effective
                .accel_profile
                .set(Some(dev.device().accel_profile()));
        }
    }

    fn set_click_method_(&self, method: ConfigClickMethod) {
        self.desired.click_method.set(Some(method));
        if let Some(dev) = self.inputdev.get()
            && dev.device().has_click_methods()
        {
            dev.device().set_click_method(method);
            self.effective
                .click_method
                .set(Some(dev.device().click_method()));
        }
    }

    fn set_enabled_leds_(&self, led: Led) {
        self.desired.enabled_leds.set(Some(led));
        if let Some(dev) = self.inputdev.get() {
            dev.device().led_update(led);
            self.effective.enabled_leds.set(Some(led));
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

    fn left_handed(&self) -> Option<bool> {
        self.effective.left_handed.get()
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.desired.left_handed.set(Some(left_handed));
        if let Some(dev) = self.inputdev.get()
            && dev.device().left_handed_available()
        {
            dev.device().set_left_handed(left_handed);
            self.effective
                .left_handed
                .set(Some(dev.device().left_handed()));
        }
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

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        let profile = match profile {
            InputDeviceAccelProfile::Flat => LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT,
            InputDeviceAccelProfile::Adaptive => LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
        };
        self.set_accel_profile_(profile);
    }

    fn accel_speed(&self) -> Option<f64> {
        self.effective.accel_speed.get()
    }

    fn set_accel_speed(&self, speed: f64) {
        self.desired.accel_speed.set(Some(speed));
        if let Some(dev) = self.inputdev.get()
            && dev.device().accel_available()
        {
            dev.device().set_accel_speed(speed);
            self.effective
                .accel_speed
                .set(Some(dev.device().accel_speed()));
        }
    }

    fn transform_matrix(&self) -> Option<TransformMatrix> {
        self.transform_matrix.get()
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(Some(matrix));
    }

    fn calibration_matrix(&self) -> Option<[[f32; 3]; 2]> {
        self.effective.calibration_matrix.get()
    }

    fn set_calibration_matrix(&self, m: [[f32; 3]; 2]) {
        self.desired.calibration_matrix.set(Some(m));
        if let Some(dev) = self.inputdev.get()
            && dev.device().has_calibration_matrix()
        {
            dev.device().set_calibration_matrix(m);
            self.effective
                .calibration_matrix
                .set(Some(dev.device().get_calibration_matrix()));
        }
    }

    fn name(&self) -> Rc<String> {
        self.name.get()
    }

    fn dev_t(&self) -> Option<c::dev_t> {
        Some(self.devnum)
    }

    fn tap_enabled(&self) -> Option<bool> {
        self.effective.tap_enabled.get()
    }

    fn set_tap_enabled(&self, enabled: bool) {
        self.desired.tap_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get()
            && dev.device().tap_available()
        {
            dev.device().set_tap_enabled(enabled);
            self.effective
                .tap_enabled
                .set(Some(dev.device().tap_enabled()));
        }
    }

    fn drag_enabled(&self) -> Option<bool> {
        self.effective.drag_enabled.get()
    }

    fn set_drag_enabled(&self, enabled: bool) {
        self.desired.drag_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get()
            && dev.device().tap_available()
        {
            dev.device().set_drag_enabled(enabled);
            self.effective
                .drag_enabled
                .set(Some(dev.device().drag_enabled()));
        }
    }

    fn drag_lock_enabled(&self) -> Option<bool> {
        self.effective.drag_lock_enabled.get()
    }

    fn set_drag_lock_enabled(&self, enabled: bool) {
        self.desired.drag_lock_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get()
            && dev.device().tap_available()
        {
            dev.device().set_drag_lock_enabled(enabled);
            self.effective
                .drag_lock_enabled
                .set(Some(dev.device().drag_lock_enabled()));
        }
    }

    fn natural_scrolling_enabled(&self) -> Option<bool> {
        self.effective.natural_scrolling_enabled.get()
    }

    fn set_natural_scrolling_enabled(&self, enabled: bool) {
        self.desired.natural_scrolling_enabled.set(Some(enabled));
        if let Some(dev) = self.inputdev.get()
            && dev.device().has_natural_scrolling()
        {
            dev.device().set_natural_scrolling_enabled(enabled);
            self.effective
                .natural_scrolling_enabled
                .set(Some(dev.device().natural_scrolling_enabled()));
        }
    }

    fn click_method(&self) -> Option<InputDeviceClickMethod> {
        let p = self.effective.click_method.get()?;
        let p = match p {
            LIBINPUT_CONFIG_CLICK_METHOD_NONE => InputDeviceClickMethod::None,
            LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS => InputDeviceClickMethod::ButtonAreas,
            LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER => InputDeviceClickMethod::Clickfinger,
            _ => return None,
        };
        Some(p)
    }

    fn set_click_method(&self, method: InputDeviceClickMethod) {
        let method = match method {
            InputDeviceClickMethod::None => LIBINPUT_CONFIG_CLICK_METHOD_NONE,
            InputDeviceClickMethod::ButtonAreas => LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS,
            InputDeviceClickMethod::Clickfinger => LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER,
        };
        self.set_click_method_(method);
    }

    fn middle_button_emulation_enabled(&self) -> Option<bool> {
        self.effective.middle_button_emulation_enabled.get()
    }

    fn set_middle_button_emulation_enabled(&self, enabled: bool) {
        self.desired
            .middle_button_emulation_enabled
            .set(Some(enabled));
        if let Some(dev) = self.inputdev.get()
            && dev.device().middle_button_emulation_available()
        {
            dev.device().set_middle_button_emulation_enabled(enabled);
            self.effective
                .middle_button_emulation_enabled
                .set(Some(dev.device().middle_button_emulation_enabled()));
        }
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
            bustype: dev.bustype(),
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
        let dials = dev.pad_num_dials();
        let mut groups = vec![];
        for n in 0..dev.pad_num_mode_groups() {
            let Some(group) = dev.pad_mode_group(n) else {
                break;
            };
            groups.push(TabletPadGroupInit {
                buttons: (0..buttons).filter(|b| group.has_button(*b)).collect(),
                rings: (0..rings).filter(|b| group.has_ring(*b)).collect(),
                strips: (0..strips).filter(|b| group.has_strip(*b)).collect(),
                dials: (0..dials).filter(|b| group.has_dial(*b)).collect(),
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
            dials,
            groups,
        }))
    }

    fn set_enabled_leds(&self, leds: Leds) {
        let led = Led(leds.0 as _);
        self.set_enabled_leds_(led);
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
