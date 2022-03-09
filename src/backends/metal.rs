mod input;
mod monitor;

use crate::async_engine::AsyncFd;
use crate::dbus::DbusError;
use crate::libinput::device::RegisteredDevice;
use crate::libinput::{LibInput, LibInputAdapter, LibInputError};
use crate::logind::{LogindError, Session};
use crate::udev::{UdevError, UdevMonitor};
use crate::utils::copyhashmap::CopyHashMap;
use crate::{AsyncQueue, CloneCell, ErrorFmt, NumCell, State, Udev};
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
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
}

pub async fn run(state: Rc<State>) {
    if let Err(e) = run_(state).await {
        log::error!("{}", ErrorFmt(e));
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
    ids: NumCell<u64>,
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
        ids: Default::default(),
    });
    let _monitor = state.eng.spawn(metal.clone().monitor_devices());
    let _events = state.eng.spawn(metal.clone().handle_libinput_events());
    if let Err(e) = metal.enumerate_devices() {
        return Err(MetalError::Enumerate(Box::new(e)));
    }
    let queue = AsyncQueue::<String>::new();
    queue.pop().await;
    Ok(())
}

struct MetalDevice {
    slot: usize,
    device_id: u64,
    devnum: c::dev_t,
    fd: CloneCell<Option<Rc<OwnedFd>>>,
    inputdev: Cell<Option<RegisteredDevice>>,
    devnode: CString,
    sysname: CString,
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

impl MetalBackend {
    fn id(&self) -> u64 {
        self.ids.fetch_add(1)
    }
}
