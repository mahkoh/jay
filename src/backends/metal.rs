mod input;
mod monitor;
mod video;

use crate::async_engine::AsyncFd;
use crate::backend::{Backend, InputDevice, InputDeviceId, InputEvent};
use crate::dbus::DbusError;
use crate::drm::drm::DrmError;
use crate::drm::gbm::GbmError;
use crate::libinput::device::RegisteredDevice;
use crate::libinput::{LibInput, LibInputAdapter, LibInputError};
use crate::logind::{LogindError, Session};
use crate::metal::video::{MetalDrmDevice, PendingDrmDevice};
use crate::udev::{UdevError, UdevMonitor};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::oserror::OsError;
use crate::utils::syncqueue::SyncQueue;
use crate::{AsyncError, CloneCell, RenderError, State, Udev};
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::future::pending;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, OwnedFd};

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
    #[error("Metal backend terminated unexpectedly")]
    UnexpectedTermination,
    #[error("Could not create GBM device")]
    GbmDevice(#[source] GbmError),
    #[error("Could not create a render context")]
    CreateRenderContex(#[source] RenderError),
    #[error("Cannot initialize connector because no CRTC is available")]
    NoCrtcForConnector,
    #[error("Cannot initialize connector because no primary plane is available")]
    NoPrimaryPlaneForConnector,
    #[error("Cannot initialize connector because no mode is available")]
    NoModeForConnector,
    #[error("Could not allocate scanout buffer")]
    ScanoutBuffer(#[source] GbmError),
    #[error("Could not create a framebuffer")]
    Framebuffer(#[source] DrmError),
    #[error("Could not import a framebuffer into EGL")]
    ImportFb(#[source] RenderError),
    #[error("Could not configure connector chain")]
    Configure(#[source] DrmError),
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
    #[error("Could not create an async fd for the drm fd")]
    CreateDrmAsyncFd(#[source] AsyncError),
}

pub async fn run(state: Rc<State>) -> MetalError {
    match run_(state).await {
        Err(e) => e,
        _ => MetalError::UnexpectedTermination,
    }
}

linear_ids!(DrmIds, DrmId);

struct MetalBackend {
    state: Rc<State>,
    udev: Rc<Udev>,
    monitor: Rc<UdevMonitor>,
    monitor_fd: AsyncFd,
    libinput: Rc<LibInput>,
    libinput_fd: AsyncFd,
    device_holder: Rc<DeviceHolder>,
    session: Session,
    drm_ids: DrmIds,
}

impl Backend for MetalBackend {}

async fn run_(state: Rc<State>) -> Result<(), MetalError> {
    let socket = match state.dbus.system() {
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
    });
    let udev = Rc::new(Udev::new()?);
    let monitor = Rc::new(udev.create_monitor()?);
    monitor.add_match_subsystem_devtype(Some("input"), None)?;
    monitor.add_match_subsystem_devtype(Some("drm"), None)?;
    monitor.enable_receiving()?;
    let libinput = Rc::new(LibInput::new(device_holder.clone())?);
    let monitor_fd = match uapi::fcntl_dupfd_cloexec(monitor.fd(), 0) {
        Ok(m) => state.eng.fd(&Rc::new(m)).unwrap(),
        Err(e) => return Err(MetalError::Dup(e.into())),
    };
    let libinput_fd = match uapi::fcntl_dupfd_cloexec(libinput.fd(), 0) {
        Ok(m) => state.eng.fd(&Rc::new(m)).unwrap(),
        Err(e) => return Err(MetalError::Dup(e.into())),
    };
    let metal = Rc::new(MetalBackend {
        state: state.clone(),
        udev,
        monitor,
        monitor_fd,
        libinput,
        libinput_fd,
        device_holder,
        session,
        drm_ids: Default::default(),
    });
    let _pause_handler = {
        let mtl = metal.clone();
        metal
            .session
            .on_pause(move |p| mtl.handle_device_pause(p))
            .unwrap()
    };
    let _resume_handler = {
        let mtl = metal.clone();
        metal
            .session
            .on_resume(move |p| mtl.handle_device_resume(p))
            .unwrap()
    };
    let _monitor = state.eng.spawn(metal.clone().monitor_devices());
    let _events = state.eng.spawn(metal.clone().handle_libinput_events());
    if let Err(e) = metal.enumerate_devices() {
        return Err(MetalError::Enumerate(Box::new(e)));
    }
    pending().await
}

struct MetalInputDevice {
    slot: usize,
    id: InputDeviceId,
    devnum: c::dev_t,
    fd: CloneCell<Option<Rc<OwnedFd>>>,
    inputdev: Cell<Option<RegisteredDevice>>,
    devnode: CString,
    _sysname: CString,
    removed: Cell<bool>,
    events: SyncQueue<InputEvent>,
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
}

#[derive(Clone)]
enum MetalDevice {
    Input(Rc<MetalInputDevice>),
    Drm(Rc<MetalDrmDevice>),
}

struct DeviceHolder {
    devices: CopyHashMap<c::dev_t, MetalDevice>,
    input_devices: RefCell<Vec<Option<Rc<MetalInputDevice>>>>,
    drm_devices: CopyHashMap<c::dev_t, Rc<MetalDrmDevice>>,
    pending_drm_devices: CopyHashMap<c::dev_t, PendingDrmDevice>,
}

impl LibInputAdapter for DeviceHolder {
    fn open(&self, path: &CStr) -> Result<OwnedFd, LibInputError> {
        let stat = match uapi::stat(path) {
            Ok(s) => s,
            Err(e) => return Err(LibInputError::Stat(e.into())),
        };
        match self.devices.get(&stat.st_rdev) {
            Some(MetalDevice::Input(d)) => match d.fd.get() {
                Some(fd) => match uapi::fcntl_dupfd_cloexec(fd.raw(), 0) {
                    Ok(fd) => Ok(fd),
                    Err(e) => Err(LibInputError::DupFd(e.into())),
                },
                _ => Err(LibInputError::DeviceUnavailable),
            },
            _ => Err(LibInputError::DeviceUnavailable),
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
        log::warn!("Metal backend does not support grabbing devices");
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
