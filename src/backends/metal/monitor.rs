use {
    crate::{
        backend::{BackendEvent, ConnectorEvent},
        backends::metal::{
            video::{FrontState, MetalDrmDeviceData, PendingDrmDevice},
            MetalBackend, MetalDevice, MetalError, MetalInputDevice,
        },
        dbus::{DbusError, TRUE},
        udev::UdevDevice,
        utils::{
            bitflags::BitflagsExt, cell_ext::CellExt, errorfmt::ErrorFmt, nonblock::set_nonblock,
        },
        video::drm::DrmMaster,
        wire_dbus::org::freedesktop::login1::session::{
            PauseDevice, ResumeDevice, TakeDeviceReply,
        },
    },
    bstr::ByteSlice,
    std::{cell::Cell, rc::Rc},
    uapi::{c, OwnedFd},
};

const DRM: &[u8] = b"drm";
const INPUT: &[u8] = b"input";
const EVENT: &[u8] = b"event";

const CARD: &[u8] = b"card";

fn is_primary_node(n: &[u8]) -> bool {
    match n.strip_prefix(CARD) {
        Some(r) => r.iter().copied().all(|c| matches!(c, b'0'..=b'9')),
        _ => false,
    }
}

impl MetalBackend {
    pub async fn monitor_devices(self: Rc<Self>) {
        loop {
            match self.state.ring.readable(&self.monitor_fd).await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for udev_monitor to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(n) if n.intersects(c::POLLERR | c::POLLHUP) => {
                    log::error!("udev_monitor fd is in an error state");
                    break;
                }
                _ => {}
            }
            while let Some(dev) = self.monitor.receive_device() {
                let action = match dev.action() {
                    Some(c) => c,
                    _ => continue,
                };
                match action.to_bytes() {
                    b"add" => self.handle_device_add(dev),
                    b"change" => self.handle_device_change(dev),
                    _ => None,
                };
            }
        }
        log::error!("Monitor task exited. Future hotplug events will be ignored.");
    }

    pub fn handle_device_pause(self: &Rc<Self>, pause: PauseDevice) {
        if pause.ty == "pause" {
            self.session.device_paused(pause.major, pause.minor);
        }
        let dev = uapi::makedev(pause.major as _, pause.minor as _);
        if pause.ty == "gone" {
            self.handle_device_removed(dev);
        } else {
            self.handle_device_paused(dev);
        }
    }

    pub fn handle_device_resume(self: &Rc<Self>, resume: ResumeDevice) {
        let dev = uapi::makedev(resume.major as _, resume.minor as _);
        let dev = match self.device_holder.devices.get(&dev) {
            Some(d) => d,
            _ => return,
        };
        match dev {
            MetalDevice::Input(id) => self.handle_input_device_resume(&id, resume.fd),
            MetalDevice::Drm(dd) => self.handle_drm_device_resume(&dd, resume.fd),
        }
    }

    fn handle_drm_device_resume(self: &Rc<Self>, dev: &Rc<MetalDrmDeviceData>, _fd: Rc<OwnedFd>) {
        log::info!("Device resumed: {}", dev.dev.devnode.to_bytes().as_bstr());
        dev.dev.paused.set(false);
        self.break_leases(dev);
        for c in dev.connectors.lock().values() {
            match c.frontend_state.get() {
                FrontState::Removed | FrontState::Disconnected | FrontState::Connected { .. } => {}
                FrontState::Unavailable => {
                    if c.lease.is_none() {
                        c.send_event(ConnectorEvent::Available);
                    }
                }
            }
        }
        if let Err(e) = self.resume_drm_device(dev) {
            log::error!("Could not resume drm device: {}", ErrorFmt(e));
        }
    }

    fn handle_input_device_resume(self: &Rc<Self>, dev: &Rc<MetalInputDevice>, fd: Rc<OwnedFd>) {
        log::info!("Device resumed: {}", dev.devnode.to_bytes().as_bstr());
        if let Some(old) = dev.fd.set(Some(fd)) {
            self.state.fdcloser.close(old);
        }
        let inputdev = match self.libinput.open(dev.devnode.as_c_str()) {
            Ok(d) => Rc::new(d),
            Err(_) => return,
        };
        inputdev.device().set_slot(dev.slot);
        dev.inputdev.set(Some(inputdev));
        dev.apply_config();
    }

    fn handle_device_removed(self: &Rc<Self>, dev: c::dev_t) {
        let dev = match self.device_holder.devices.remove(&dev) {
            Some(d) => d,
            _ => return,
        };
        match dev {
            MetalDevice::Input(id) => self.handle_input_device_removed(&id),
            MetalDevice::Drm(dd) => self.handle_drm_device_removed(&dd),
        }
    }

    fn handle_drm_device_removed(self: &Rc<Self>, dev: &Rc<MetalDrmDeviceData>) {
        log::info!("Device removed: {}", dev.dev.devnode.to_bytes().as_bstr());
    }

    fn handle_input_device_removed(self: &Rc<Self>, dev: &Rc<MetalInputDevice>) {
        dev.pre_pause();
        log::info!("Device removed: {}", dev.devnode.to_bytes().as_bstr());
        self.device_holder.input_devices.borrow_mut()[dev.slot] = None;
        dev.fd.set(None);
        if let Some(rd) = dev.inputdev.take() {
            rd.device().unset_slot();
        }
        dev.removed.set(true);
        if let Some(cb) = dev.cb.take() {
            cb();
        }
    }

    fn handle_device_paused(self: &Rc<Self>, dev: c::dev_t) {
        let dev = match self.device_holder.devices.get(&dev) {
            Some(d) => d,
            _ => return,
        };
        match dev {
            MetalDevice::Input(id) => self.handle_input_device_paused(&id),
            MetalDevice::Drm(dd) => self.handle_drm_device_paused(&dd),
        }
    }

    fn handle_drm_device_paused(self: &Rc<Self>, dev: &Rc<MetalDrmDeviceData>) {
        dev.dev.paused.set(true);
        for c in dev.connectors.lock().values() {
            match c.frontend_state.get() {
                FrontState::Removed
                | FrontState::Disconnected
                | FrontState::Unavailable
                | FrontState::Connected { non_desktop: false } => {}
                FrontState::Connected { non_desktop: true } => {
                    c.send_event(ConnectorEvent::Unavailable);
                }
            }
        }
        for (lease_id, lease) in dev.dev.leases.lock().drain() {
            dev.dev.leases_to_break.set(lease_id, lease);
        }
        log::info!("Device paused: {}", dev.dev.devnode.to_bytes().as_bstr());
    }

    fn handle_input_device_paused(self: &Rc<Self>, dev: &Rc<MetalInputDevice>) {
        log::info!("Device paused: {}", dev.devnode.to_bytes().as_bstr());
        dev.pre_pause();
        if let Some(rd) = dev.inputdev.take() {
            rd.device().unset_slot();
        }
    }

    fn handle_device_add(self: &Rc<Self>, dev: UdevDevice) -> Option<()> {
        let ss = dev.subsystem()?;
        match ss.to_bytes() {
            INPUT => self.handle_input_device_add(dev),
            DRM => self.handle_drm_add(dev),
            _ => None,
        }
    }

    fn handle_input_device_add(self: &Rc<Self>, dev: UdevDevice) -> Option<()> {
        let sysname = dev.sysname()?;
        if sysname.to_bytes().starts_with(EVENT) {
            self.add_input_device(&dev);
        }
        None
    }

    fn handle_drm_add(self: &Rc<Self>, dev: UdevDevice) -> Option<()> {
        let sysname = dev.sysname()?;
        if !is_primary_node(sysname.to_bytes()) {
            return None;
        }
        let devnum = dev.devnum();
        let devnode = dev.devnode()?;
        let id = self.state.drm_dev_ids.next();
        let model = dev
            .parent()
            .ok()
            .and_then(|dev| dev.model().map(|s| s.to_string_lossy().into_owned()));
        log::info!(
            "Device added: {} ({})",
            devnode.to_bytes().as_bstr(),
            model.as_deref().unwrap_or("unknown"),
        );
        let dev = PendingDrmDevice {
            id,
            devnum,
            devnode: devnode.to_owned(),
        };
        self.device_holder.pending_drm_devices.set(devnum, dev);
        let slf = self.clone();
        self.get_device(devnum, move |res| {
            let dev = match slf.device_holder.pending_drm_devices.remove(&devnum) {
                Some(d) if d.id == id => d,
                _ => return,
            };
            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Could not take control of drm device: {}", ErrorFmt(e));
                    return;
                }
            };
            let master = Rc::new(DrmMaster::new(&slf.state.ring, res.fd.clone()));
            let dev = match slf.create_drm_device(dev, &master) {
                Ok(d) => d,
                Err(e) => {
                    log::error!("Could not initialize drm device: {}", ErrorFmt(e));
                    return;
                }
            };
            slf.device_holder
                .drm_devices
                .set(dev.dev.devnum, dev.clone());
            slf.device_holder
                .devices
                .set(dev.dev.devnum, MetalDevice::Drm(dev.clone()));
        });
        None
    }

    fn handle_device_change(self: &Rc<Self>, dev: UdevDevice) -> Option<()> {
        let ss = dev.subsystem()?;
        log::info!("Device changed: {}", dev.devnode()?.to_bytes().as_bstr());
        match ss.to_bytes() {
            DRM => self.handle_drm_change(dev),
            _ => None,
        }
    }

    pub fn enumerate_devices(self: &Rc<Self>) -> Result<(), MetalError> {
        let mut enumerate = self.udev.create_enumerate()?;
        enumerate.add_match_subsystem(INPUT)?;
        enumerate.add_match_subsystem(DRM)?;
        enumerate.scan_devices()?;
        let mut entry_opt = enumerate.get_list_entry()?;
        while let Some(entry) = entry_opt.take() {
            if let Ok(dev) = self.udev.create_device_from_syspath(entry.name()) {
                self.handle_device_add(dev);
            }
            entry_opt = entry.next();
        }
        Ok(())
    }

    fn add_input_device(self: &Rc<Self>, dev: &UdevDevice) -> Option<()> {
        if !dev.is_initialized() {
            return None;
        }
        let slf = self.clone();
        let device_id = self.state.input_device_ids.next();
        let devnum = dev.devnum();
        let devnode = dev.devnode()?;
        let sysname = dev.sysname()?;
        log::info!("Device added: {}", devnode.to_bytes().as_bstr());
        let mut slots = self.device_holder.input_devices.borrow_mut();
        let slot = 'slot: {
            for (i, s) in slots.iter().enumerate() {
                if s.is_none() {
                    break 'slot i;
                }
            }
            slots.push(None);
            slots.len() - 1
        };
        let dev = Rc::new(MetalInputDevice {
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
            name: Default::default(),
            pressed_keys: Default::default(),
            pressed_buttons: Default::default(),
            desired: Default::default(),
            transform_matrix: Default::default(),
            effective: Default::default(),
        });
        slots[slot] = Some(dev.clone());
        self.device_holder
            .devices
            .set(devnum, MetalDevice::Input(dev));
        self.get_device(devnum, move |res| {
            let id = &slf.device_holder.devices;
            let mut slots = slf.device_holder.input_devices.borrow_mut();
            let dev = 'dev: {
                if let Some(dev) = slots[slot].clone() {
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
            if let Err(e) = set_nonblock(res.fd.raw()) {
                log::error!("Could set input fd to non-blocking: {}", ErrorFmt(e));
                return;
            }
            dev.fd.set(Some(res.fd.clone()));
            let inputdev = match slf.libinput.open(dev.devnode.as_c_str()) {
                Ok(d) => Rc::new(d),
                Err(_) => return,
            };
            inputdev.device().set_slot(slot);
            dev.name.set(Rc::new(inputdev.device().name()));
            dev.inputdev.set(Some(inputdev));
            dev.apply_config();
            slf.state
                .backend_events
                .push(BackendEvent::NewInputDevice(dev.clone()));
        });
        None
    }

    fn get_device<F>(self: &Rc<Self>, dev: c::dev_t, f: F)
    where
        F: FnOnce(Result<&TakeDeviceReply, DbusError>) + 'static,
    {
        self.device_holder.num_pending_devices.fetch_add(1);
        let slf = self.clone();
        self.session.get_device(dev, move |res| {
            let rem = slf.device_holder.num_pending_devices.fetch_sub(1);
            f(res);
            if rem == 1 {
                slf.state
                    .backend_events
                    .push(BackendEvent::DevicesEnumerated);
                // Set to 1 to ensure this branch is never taken again.
                slf.device_holder.num_pending_devices.set(1);
            }
        })
    }
}
