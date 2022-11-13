use {
    crate::{
        backend::{BackendDrmDevice, DrmDeviceId, DrmEvent},
        state::{DrmDevData, State},
        udev::{Udev, UdevDeviceType},
        utils::{asyncevent::AsyncEvent, errorfmt::ErrorFmt},
    },
    jay_config::PciId,
    std::{cell::Cell, rc::Rc},
};

pub fn handle(state: &Rc<State>, dev: Rc<dyn BackendDrmDevice>) {
    let id = dev.id();
    let mut syspath = None;
    let mut devnode = None;
    let mut vendor = None;
    let mut model = None;
    let mut pci_id = None;
    'properties: {
        let udev = match Udev::new() {
            Ok(udev) => Rc::new(udev),
            Err(e) => {
                log::error!("Could not create a udev instance: {}", e);
                break 'properties;
            }
        };
        let odev = match udev.create_device_from_devnum(UdevDeviceType::Character, dev.dev_t()) {
            Ok(dev) => dev,
            Err(e) => {
                log::error!("{}", ErrorFmt(e));
                break 'properties;
            }
        };
        let dev = match odev.parent() {
            Ok(dev) => dev,
            Err(e) => {
                log::error!("{}", ErrorFmt(e));
                break 'properties;
            }
        };
        syspath = dev.syspath().map(|s| s.to_string_lossy().into_owned());
        vendor = dev.vendor().map(|s| s.to_string_lossy().into_owned());
        model = dev.model().map(|s| s.to_string_lossy().into_owned());
        devnode = odev.devnode().map(|s| s.to_string_lossy().into_owned());
        'get_pci_id: {
            let id = match dev.pci_id() {
                Some(id) => id,
                _ => break 'get_pci_id,
            };
            let id = id.to_string_lossy();
            let colon = match id.find(':') {
                Some(pos) => pos,
                _ => break 'get_pci_id,
            };
            let vendor = &id[..colon];
            let model = &id[colon + 1..];
            let vendor = match u32::from_str_radix(vendor, 16) {
                Ok(v) => v,
                _ => break 'get_pci_id,
            };
            let model = match u32::from_str_radix(model, 16) {
                Ok(v) => v,
                _ => break 'get_pci_id,
            };
            pci_id = Some(PciId { vendor, model });
        }
    }
    let data = Rc::new(DrmDevData {
        dev: dev.clone(),
        handler: Cell::new(None),
        connectors: Default::default(),
        syspath,
        devnode,
        vendor,
        model,
        pci_id,
    });
    let oh = DrvDevHandler {
        id,
        state: state.clone(),
        data: data.clone(),
    };
    let future = state.eng.spawn(oh.handle());
    data.handler.set(Some(future));
    if state.drm_devs.set(id, data).is_some() {
        panic!("Drm device id has been reused");
    }
}

struct DrvDevHandler {
    id: DrmDeviceId,
    state: Rc<State>,
    data: Rc<DrmDevData>,
}

impl DrvDevHandler {
    async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.data.dev.on_change(Rc::new(move || ae.trigger()));
        }
        if let Some(config) = self.state.config.get() {
            config.new_drm_dev(self.id);
        }
        'outer: loop {
            #[allow(clippy::never_loop)]
            while let Some(event) = self.data.dev.event() {
                match event {
                    DrmEvent::Removed => break 'outer,
                }
            }
            ae.triggered().await;
        }
        if !self.data.connectors.is_empty() {
            panic!("DRM device removed before its connectors");
        }
        if let Some(config) = self.state.config.get() {
            config.del_drm_dev(self.id);
        }
        self.data.handler.set(None);
        self.state.drm_devs.remove(&self.id);
    }
}
