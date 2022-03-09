use std::cell::Cell;
use crate::async_engine::FdStatus;
use crate::dbus::TRUE;
use crate::metal::{MetalBackend, MetalDevice, MetalError};
use crate::udev::UdevDevice;
use crate::ErrorFmt;
use std::rc::Rc;
use crate::backend::BackendEvent;

impl MetalBackend {
    pub async fn monitor_devices(self: Rc<Self>) {
        loop {
            match self.monitor_fd.readable().await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for udev_monitor to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(FdStatus::Err) => {
                    log::error!("udev_monitor fd is in an error state");
                    break;
                }
                _ => {}
            }
            while let Some(dev) = self.monitor.receive_device() {
                log::info!("x {:?}", dev.devnode());
            }
        }
        log::error!("Monitor task exited. Future hotplug events will be ignored.");
    }

    pub fn enumerate_devices(self: &Rc<Self>) -> Result<(), MetalError> {
        let mut enumerate = self.udev.create_enumerate()?;
        enumerate.add_match_subsystem("input")?;
        enumerate.scan_devices()?;
        let mut entry_opt = enumerate.get_list_entry()?;
        while let Some(entry) = entry_opt.take() {
            'inner: {
                let device = match self.udev.create_device_from_syspath(entry.name()) {
                    Ok(d) => d,
                    _ => break 'inner,
                };
                let sysname = match device.sysname() {
                    Ok(s) => s,
                    _ => break 'inner,
                };
                if sysname.to_bytes().starts_with(b"event") {
                    self.add_input_device(&device);
                }
            }
            entry_opt = entry.next();
        }
        Ok(())
    }

    fn add_input_device(self: &Rc<Self>, dev: &UdevDevice) {
        if !dev.is_initialized() {
            return;
        }
        let slf = self.clone();
        let device_id = self.state.input_device_ids.next();
        let devnum = dev.devnum();
        let devnode = match dev.devnode() {
            Ok(n) => n,
            Err(e) => {
                log::error!("Could not retrieve devnode of udev device: {}", ErrorFmt(e));
                return;
            }
        };
        let sysname = match dev.sysname() {
            Ok(n) => n,
            Err(e) => {
                log::error!("Could not retrieve sysname of udev device: {}", ErrorFmt(e));
                return;
            }
        };
        let mut slots = self.device_holder.input_devices_.borrow_mut();
        let slot = 'slot: {
            for (i, s) in slots.iter().enumerate() {
                if s.is_none() {
                    break 'slot i;
                }
            }
            slots.push(None);
            slots.len() - 1
        };
        let dev = Rc::new(MetalDevice {
            slot,
            id: device_id,
            devnum,
            fd: Default::default(),
            inputdev: Default::default(),
            devnode: devnode.to_owned(),
            _sysname: sysname.to_owned(),
            removed: Cell::new(false),
            events: Default::default(),
            cb: Default::default(),
        });
        slots[slot] = Some(dev.clone());
        self.device_holder.input_devices.set(devnum, dev);
        self.session.get_device(devnum, move |res| {
            let id = &slf.device_holder.input_devices;
            let mut slots = slf.device_holder.input_devices_.borrow_mut();
            let dev = 'dev: {
                if let Some(dev) = id.get(&devnum) {
                    if dev.id == device_id {
                        break 'dev dev;
                    }
                }
                return;
            };
            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Could not take control of input device: {}", ErrorFmt(e));
                    slots[dev.slot] = None;
                    id.remove(&devnum);
                    return;
                }
            };
            if res.inactive == TRUE {
                return;
            }
            dev.fd.set(Some(res.fd.clone()));
            let inputdev = match slf.libinput.open(dev.devnode.as_c_str()) {
                Ok(d) => d,
                Err(_) => {
                    slots[dev.slot] = None;
                    id.remove(&devnum);
                    return;
                }
            };
            inputdev.device().set_slot(slot);
            dev.inputdev.set(Some(inputdev));
            slf.state.backend_events.push(BackendEvent::NewInputDevice(dev.clone()));
        });
    }
}
