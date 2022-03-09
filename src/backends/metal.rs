mod input;
mod monitor;

use crate::async_engine::AsyncFd;
use crate::dbus::DbusError;
use crate::libinput::device::RegisteredDevice;
use crate::libinput::{LibInput, LibInputAdapter, LibInputError};
use crate::logind::{LogindError, Session};
use crate::udev::{UdevError, UdevMonitor};
use crate::utils::copyhashmap::CopyHashMap;
use crate::{CloneCell, State, Udev};
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::future::pending;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, OwnedFd};
use crate::backend::{Backend, InputDevice, InputDeviceId, InputEvent};
use crate::utils::syncqueue::SyncQueue;

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
}

pub async fn run(state: Rc<State>) -> MetalError {
    match run_(state).await {
        Err(e) => e,
        _ => MetalError::UnexpectedTermination,
    }
}

struct MetalBackend {
    state: Rc<State>,
    udev: Rc<Udev>,
    monitor: Rc<UdevMonitor>,
    monitor_fd: AsyncFd,
    libinput: Rc<LibInput>,
    libinput_fd: AsyncFd,
    device_holder: Rc<DeviceHolder>,
    session: Session,
}

impl Backend for MetalBackend {

}

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
        input_devices: Default::default(),
        input_devices_: Default::default(),
    });
    let udev = Rc::new(Udev::new()?);
    let monitor = Rc::new(udev.create_monitor()?);
    monitor.add_match_subsystem_devtype(Some("input"), None)?;
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
    });
    let _monitor = state.eng.spawn(metal.clone().monitor_devices());
    let _events = state.eng.spawn(metal.clone().handle_libinput_events());
    if let Err(e) = metal.enumerate_devices() {
        return Err(MetalError::Enumerate(Box::new(e)));
    }
    pending().await
}

struct MetalDevice {
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

struct DeviceHolder {
    input_devices: CopyHashMap<c::dev_t, Rc<MetalDevice>>,
    input_devices_: RefCell<Vec<Option<Rc<MetalDevice>>>>,
}

impl LibInputAdapter for DeviceHolder {
    fn open(&self, path: &CStr) -> Result<OwnedFd, LibInputError> {
        let stat = match uapi::stat(path) {
            Ok(s) => s,
            Err(e) => return Err(LibInputError::Stat(e.into())),
        };
        match self.input_devices.get(&stat.st_rdev) {
            Some(d) => match d.fd.get() {
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

impl InputDevice for MetalDevice {
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

impl MetalDevice {
    fn event(&self, event: InputEvent) {
        self.events.push(event);
        if let Some(cb) = self.cb.get() {
            cb();
        }
    }
}
