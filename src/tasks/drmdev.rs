use {
    crate::{
        backend::{BackendDrmDevice, DrmDeviceId, DrmEvent},
        buffer_id_device::BufferIdDeviceDyn,
        ifs::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1Global,
        state::{DrmDevData, State},
        tasks::udev_utils::udev_props,
        utils::{asyncevent::AsyncEvent, errorfmt::ErrorFmt},
    },
    std::{cell::Cell, rc::Rc},
};

pub fn handle(state: &Rc<State>, dev: Rc<dyn BackendDrmDevice>) {
    let id = dev.id();
    let dev_t = dev.dev_t();
    let props = udev_props(dev_t, 1);
    let lease_global = Rc::new(WpDrmLeaseDeviceV1Global {
        name: state.globals.name(),
        device: id,
        bindings: Default::default(),
    });
    state.add_global(&lease_global);
    let copy_device = state.copy_device_registry.get(id, dev_t).and_then(|d| {
        d.create_device()
            .inspect_err(|e| {
                let maj = uapi::major(dev_t);
                let min = uapi::minor(dev_t);
                log::warn!(
                    "Could not create copy device for {maj}:{min}: {}",
                    ErrorFmt(e),
                );
            })
            .ok()
    });
    let mut id_device = copy_device.clone().map(|d| d as Rc<dyn BufferIdDeviceDyn>);
    if id_device.is_none() {
        id_device = state
            .buffer_id_device_registry
            .get(dev_t)
            .map(|v| v as Rc<dyn BufferIdDeviceDyn>);
    }
    let data = Rc::new(DrmDevData {
        id,
        dev: dev.clone(),
        handler: Cell::new(None),
        connectors: Default::default(),
        dev_t,
        syspath: props.syspath,
        devnode: props.devnode,
        vendor: props.vendor,
        model: props.model,
        pci_id: props.pci_id,
        lease_global,
        copy_device,
        id_device,
    });
    let oh = DrvDevHandler {
        id,
        state: state.clone(),
        data: data.clone(),
    };
    let future = state.eng.spawn("drmdev handler", oh.handle());
    data.handler.set(Some(future));
    if state.drm_devs.set(id, data).is_some() {
        panic!("Drm device id has been reused");
    }
    state.dmabuf_feedback.update();
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
        self.log_gfx_api();
        'outer: loop {
            while let Some(event) = self.data.dev.event() {
                match event {
                    DrmEvent::Removed => break 'outer,
                    DrmEvent::GfxApiChanged => self.log_gfx_api(),
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
        self.data.lease_global.bindings.clear();
        let _ = self.state.remove_global(&self.data.lease_global);
        self.data.handler.set(None);
        self.state.drm_devs.remove(&self.id);
    }

    fn log_gfx_api(&self) {
        let api = self.data.dev.gfx_api();
        log::info!(
            "Using {:?} for device {}",
            api,
            self.data.devnode.as_deref().unwrap_or(""),
        )
    }
}
