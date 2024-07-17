use {
    crate::{
        backend,
        client::{Client, ClientError},
        compositor::MAX_EXTENTS,
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        state::{ConnectorData, DrmDevData, OutputData},
        tree::{OutputNode, VrrMode},
        utils::{gfx_api_ext::GfxApiExt, transform_ext::TransformExt},
        wire::{jay_randr::*, JayRandrId},
    },
    jay_config::video::{GfxApi, Transform, VrrMode as ConfigVrrMode},
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayRandr {
    pub id: JayRandrId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

const VRR_CAPABLE_SINCE: Version = Version(2);

impl JayRandr {
    pub fn new(id: JayRandrId, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        }
    }

    fn send_global(&self) {
        self.client.event(Global {
            self_id: self.id,
            default_gfx_api: self.client.state.default_gfx_api.get().to_str(),
        })
    }

    fn send_drm_device(&self, data: &DrmDevData) {
        self.client.event(DrmDevice {
            self_id: self.id,
            id: data.dev.id().raw() as _,
            syspath: data.syspath.as_deref().unwrap_or_default(),
            vendor: data.pci_id.map(|p| p.vendor).unwrap_or_default(),
            vendor_name: data.vendor.as_deref().unwrap_or_default(),
            model: data.pci_id.map(|p| p.model).unwrap_or_default(),
            model_name: data.model.as_deref().unwrap_or_default(),
            devnode: data.devnode.as_deref().unwrap_or_default(),
            gfx_api: data.dev.gtx_api().to_str(),
            render_device: data.dev.is_render_device() as _,
        });
    }

    fn send_connector(&self, data: &ConnectorData) {
        self.client.event(Connector {
            self_id: self.id,
            id: data.connector.id().raw() as _,
            drm_device: data
                .drm_dev
                .as_ref()
                .map(|d| d.dev.id().raw() as _)
                .unwrap_or_default(),
            enabled: data.connector.enabled() as _,
            name: &data.name,
        });
        let Some(output) = self.client.state.outputs.get(&data.connector.id()) else {
            return;
        };
        let node = match &output.node {
            Some(n) => n,
            None => {
                self.client.event(NonDesktopOutput {
                    self_id: self.id,
                    manufacturer: &output.monitor_info.manufacturer,
                    product: &output.monitor_info.product,
                    serial_number: &output.monitor_info.serial_number,
                    width_mm: output.monitor_info.width_mm,
                    height_mm: output.monitor_info.height_mm,
                });
                return;
            }
        };
        let global = &node.global;
        let pos = global.pos.get();
        self.client.event(Output {
            self_id: self.id,
            scale: global.persistent.scale.get().to_wl(),
            width: pos.width(),
            height: pos.height(),
            x: pos.x1(),
            y: pos.y1(),
            transform: global.persistent.transform.get().to_wl(),
            manufacturer: &output.monitor_info.manufacturer,
            product: &output.monitor_info.product,
            serial_number: &output.monitor_info.serial_number,
            width_mm: global.width_mm,
            height_mm: global.height_mm,
        });
        if self.version >= VRR_CAPABLE_SINCE {
            self.client.event(VrrState {
                self_id: self.id,
                capable: output.monitor_info.vrr_capable as _,
                enabled: node.schedule.vrr_enabled() as _,
                mode: node.global.persistent.vrr_mode.get().to_config().0,
            });
            if let Some(hz) = node.global.persistent.vrr_cursor_hz.get() {
                self.client.event(VrrCursorHz {
                    self_id: self.id,
                    hz,
                });
            }
        }
        let current_mode = global.mode.get();
        for mode in &global.modes {
            self.client.event(Mode {
                self_id: self.id,
                width: mode.width,
                height: mode.height,
                refresh_rate_millihz: mode.refresh_rate_millihz,
                current: (mode == &current_mode) as _,
            });
        }
    }

    fn send_error(&self, msg: &str) {
        self.client.event(Error {
            self_id: self.id,
            msg,
        });
    }

    fn get_device(&self, name: &str) -> Option<Rc<DrmDevData>> {
        let mut candidates = vec![];
        for dev in self.client.state.drm_devs.lock().values() {
            if let Some(node) = &dev.devnode {
                if node.ends_with(name) {
                    candidates.push(dev.clone());
                }
            }
        }
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }
        if candidates.len() == 0 {
            self.send_error(&format!("Found no device matching `{}`", name));
        } else {
            self.send_error(&format!("The device suffix `{}` is ambiguous", name));
        }
        None
    }

    fn get_connector(&self, name: &str) -> Option<Rc<ConnectorData>> {
        let namelc = name.to_ascii_lowercase();
        for c in self.client.state.connectors.lock().values() {
            if c.name.to_ascii_lowercase() == namelc {
                return Some(c.clone());
            }
        }
        self.send_error(&format!("Found no connector matching `{}`", name));
        None
    }

    fn get_output(&self, name: &str) -> Option<Rc<OutputData>> {
        let namelc = name.to_ascii_lowercase();
        for c in self.client.state.outputs.lock().values() {
            if c.connector.name.to_ascii_lowercase() == namelc {
                return Some(c.clone());
            }
        }
        if let Some(c) = self.get_connector(name) {
            self.send_error(&format!("Connector {} is not connected", c.name));
        }
        None
    }

    fn get_output_node(&self, name: &str) -> Option<Rc<OutputNode>> {
        let output = self.get_output(name)?;
        match output.node.clone() {
            Some(n) => return Some(n),
            _ => self.send_error(&format!(
                "Display connected to {} is not a desktop display",
                output.connector.name
            )),
        }
        None
    }
}

impl JayRandrRequestHandler for JayRandr {
    type Error = JayRandrError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get(&self, _req: Get, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let state = &self.client.state;
        self.send_global();
        for dev in state.drm_devs.lock().values() {
            self.send_drm_device(dev);
        }
        for connector in state.connectors.lock().values() {
            self.send_connector(connector);
        }
        Ok(())
    }

    fn set_api(&self, req: SetApi, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(dev) = self.get_device(req.dev) else {
            return Ok(());
        };
        let Some(api) = GfxApi::from_str_lossy(req.api) else {
            self.send_error(&format!("Unknown API `{}`", req.api));
            return Ok(());
        };
        dev.dev.set_gfx_api(api);
        Ok(())
    }

    fn make_render_device(
        &self,
        req: MakeRenderDevice,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(dev) = self.get_device(req.dev) else {
            return Ok(());
        };
        dev.make_render_device();
        Ok(())
    }

    fn set_direct_scanout(
        &self,
        req: SetDirectScanout,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(dev) = self.get_device(req.dev) else {
            return Ok(());
        };
        dev.dev.set_direct_scanout_enabled(req.enabled != 0);
        Ok(())
    }

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        let Some(transform) = Transform::from_wl(req.transform) else {
            self.send_error(&format!("Unknown transform {}", req.transform));
            return Ok(());
        };
        c.update_transform(transform);
        Ok(())
    }

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.set_preferred_scale(Scale::from_wl(req.scale));
        Ok(())
    }

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_output(req.output) else {
            return Ok(());
        };
        c.connector.connector.set_mode(backend::Mode {
            width: req.width,
            height: req.height,
            refresh_rate_millihz: req.refresh_rate_millihz,
        });
        Ok(())
    }

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        if req.x < 0 || req.y < 0 {
            self.send_error("x and y cannot be less than 0");
            return Ok(());
        }
        if req.x > MAX_EXTENTS || req.y > MAX_EXTENTS {
            self.send_error(&format!("x and y cannot be greater than {MAX_EXTENTS}"));
            return Ok(());
        }
        c.set_position(req.x, req.y);
        Ok(())
    }

    fn set_enabled(&self, req: SetEnabled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_connector(req.output) else {
            return Ok(());
        };
        c.connector.set_enabled(req.enabled != 0);
        Ok(())
    }

    fn set_non_desktop(&self, req: SetNonDesktop<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_connector(req.output) else {
            return Ok(());
        };
        let non_desktop = match req.non_desktop {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        };
        c.connector.set_non_desktop_override(non_desktop);
        Ok(())
    }

    fn set_vrr_mode(&self, req: SetVrrMode<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(mode) = VrrMode::from_config(ConfigVrrMode(req.mode)) else {
            return Err(JayRandrError::UnknownVrrMode(req.mode));
        };
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.global.persistent.vrr_mode.set(mode);
        c.update_vrr_state();
        return Ok(());
    }

    fn set_vrr_cursor_hz(
        &self,
        req: SetVrrCursorHz<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.schedule.set_cursor_hz(req.hz);
        Ok(())
    }
}

object_base! {
    self = JayRandr;
    version = self.version;
}

impl Object for JayRandr {}

simple_add_obj!(JayRandr);

#[derive(Debug, Error)]
pub enum JayRandrError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown VRR mode {0}")]
    UnknownVrrMode(u32),
}
efrom!(JayRandrError, ClientError);
