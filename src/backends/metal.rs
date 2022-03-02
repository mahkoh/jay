use crate::dbus::DbusError;
use crate::logind::{LogindError, Session};
use crate::{AsyncQueue, ErrorFmt, State, Udev};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::async_engine::{AsyncFd, FdStatus};
use crate::libinput::{LibInput, LibInputError};
use crate::udev::{UdevError, UdevMonitor};

#[derive(Debug, Error)]
pub enum MetalError {
    #[error("Could not connect to the dbus system socket")]
    DbusSystemSocket(#[source] DbusError),
    #[error("Could not retrieve the logind session")]
    LogindSession(#[source] LogindError),
    #[error("Could not take control of the logind session")]
    TakeControl(#[source] LogindError),
    #[error(transparent)]
    Udev(#[from] UdevError),
    #[error(transparent)]
    LibInput(#[from] LibInputError),
    #[error("Dupfd failed")]
    Dup(#[source] std::io::Error),
}

pub async fn run(state: Rc<State>) {
    if let Err(e) = run_(state).await {
        log::error!("{}", ErrorFmt(e));
    }
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
    // if let Err(e) = session.take_control().await {
    //     return Err(MetalError::TakeControl(e));
    // }
    let udev = Rc::new(Udev::new()?);
    let monitor = Rc::new(udev.create_monitor()?);
    monitor.add_match_subsystem_devtype(Some("input"), None)?;
    monitor.enable_receiving()?;
    let libinput = Rc::new(LibInput::new()?);
    let monitor_fd = match uapi::fcntl_dupfd_cloexec(monitor.fd(), 0) {
        Ok(m) => state.eng.fd(&Rc::new(m)).unwrap(),
        Err(e) => return Err(MetalError::Dup(e.into())),
    };
    let metal = Rc::new(MetalBackend {
        state: state.clone(),
        udev,
        monitor,
        monitor_fd,
        libinput,
    });
    let _monitor = state.eng.spawn(metal.clone().monitor_devices());
    let mut queue = AsyncQueue::<String>::new();
    queue.pop().await;
    Ok(())
    // let libinput_fd = match uapi::fcntl_dupfd_cloexec(monitor.fd(), 0) {
    //     Ok(m) => m,
    //     Err(e) => Err(MetalError::Dup(e.into())),
    // };
    // let mut enumerate = udev.create_enumerate()?;
    // enumerate.add_match_subsystem("input")?;
    // enumerate.scan_devices()?;
    // let mut entry_opt = enumerate.get_list_entry()?;
    // while let Some(entry) = entry_opt {
    //     let device = udev.create_device_from_syspath(entry.name()?)?;
    //     if device.sysname()?.to_bytes().starts_with(b"event") {
    //         let devnode = device.devnode()?;
    //     }
    // }
}

struct MetalBackend {
    state: Rc<State>,
    udev: Rc<Udev>,
    monitor: Rc<UdevMonitor>,
    monitor_fd: AsyncFd,
    libinput: Rc<LibInput>,
    libinput_fd: AsyncFd,
}

impl MetalBackend {
    async fn monitor_devices(self: Rc<Self>) {
        loop {
            match self.monitor_fd.readable().await {
                Err(e) => {
                    log::error!("Cannot wait for udev_monitor to become readable: {}", ErrorFmt(e));
                    break;
                }
                Ok(FdStatus::Err) => {
                    log::error!("udev_monitor fd is in an error state");
                    break;
                }
                _ => { },
            }
            while let Some(dev) = self.monitor.receive_device() {
                log::info!("x {:?}", dev.devnode());
            }
        }
        log::error!("Monitor task exited. Future hotplug events will be ignored.");
    }

    async fn handle_libinput_events(self: Rc<Self>) {
        loop {
            match self.libinput_fd.readable().await {
                Err(e) => {
                    log::error!("Cannot wait for udev_monitor to become readable: {}", ErrorFmt(e));
                    break;
                }
                Ok(FdStatus::Err) => {
                    log::error!("udev_monitor fd is in an error state");
                    break;
                }
                _ => { },
            }
            self.libinput.fd()
        }
        log::error!("Monitor task exited. Future hotplug events will be ignored.");
    }
}
