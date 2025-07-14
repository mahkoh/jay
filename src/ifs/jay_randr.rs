use {
    crate::{
        backend::{self, BackendColorSpace, BackendTransferFunction},
        client::{Client, ClientError},
        compositor::MAX_EXTENTS,
        format::named_formats,
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        state::{ConnectorData, DrmDevData, OutputData, State},
        tree::{OutputNode, TearingMode, VrrMode},
        utils::{errorfmt::ErrorFmt, gfx_api_ext::GfxApiExt, transform_ext::TransformExt},
        wire::{JayRandrId, jay_randr::*},
    },
    jay_config::video::{
        GfxApi, TearingMode as ConfigTearingMode, Transform, VrrMode as ConfigVrrMode,
    },
    linearize::LinearizeExt,
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayRandr {
    pub id: JayRandrId,
    pub client: Rc<Client>,
    pub state: Rc<State>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

const VRR_CAPABLE_SINCE: Version = Version(2);
const TEARING_SINCE: Version = Version(3);
const FORMAT_SINCE: Version = Version(8);
const FLIP_MARGIN_SINCE: Version = Version(10);
const COLORIMETRY_SINCE: Version = Version(15);
const BRIGHTNESS_SINCE: Version = Version(16);

impl JayRandr {
    pub fn new(id: JayRandrId, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            state: client.state.clone(),
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
        let state = data.state.get();
        self.client.event(Connector {
            self_id: self.id,
            id: data.connector.id().raw() as _,
            drm_device: data
                .drm_dev
                .as_ref()
                .map(|d| d.dev.id().raw() as _)
                .unwrap_or_default(),
            enabled: state.enabled as _,
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
                    manufacturer: &output.monitor_info.output_id.manufacturer,
                    product: &output.monitor_info.output_id.model,
                    serial_number: &output.monitor_info.output_id.serial_number,
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
            manufacturer: &output.monitor_info.output_id.manufacturer,
            product: &output.monitor_info.output_id.model,
            serial_number: &output.monitor_info.output_id.serial_number,
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
        if self.version >= TEARING_SINCE {
            self.client.event(TearingState {
                self_id: self.id,
                mode: node.global.persistent.tearing_mode.get().to_config().0,
            });
        }
        if self.version >= FORMAT_SINCE {
            let current = node.global.format.get();
            self.client.event(FbFormat {
                self_id: self.id,
                name: current.name,
                current: 1,
            });
            for &format in &*node.global.formats.get() {
                if format != current {
                    self.client.event(FbFormat {
                        self_id: self.id,
                        name: format.name,
                        current: 0,
                    });
                }
            }
        }
        if self.version >= FLIP_MARGIN_SINCE {
            if let Some(margin_ns) = node.flip_margin_ns.get() {
                self.client.event(FlipMargin {
                    self_id: self.id,
                    margin_ns,
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
        if self.version >= COLORIMETRY_SINCE {
            for tf in &node.global.transfer_functions {
                self.client.event(SupportedTransferFunction {
                    self_id: self.id,
                    transfer_function: tf.name(),
                });
            }
            self.client.event(CurrentTransferFunction {
                self_id: self.id,
                transfer_function: node.global.btf.get().name(),
            });
            for cs in &node.global.color_spaces {
                self.client.event(SupportedColorSpace {
                    self_id: self.id,
                    color_space: cs.name(),
                });
            }
            self.client.event(CurrentColorSpace {
                self_id: self.id,
                color_space: node.global.bcs.get().name(),
            });
        }
        if self.version >= BRIGHTNESS_SINCE {
            if let Some(lum) = node.global.luminance {
                self.client.event(BrightnessRange {
                    self_id: self.id,
                    min: lum.min,
                    max: lum.max,
                    max_fall: lum.max_fall,
                });
            }
            if let Some(lux) = node.global.persistent.brightness.get() {
                self.client.event(Brightness {
                    self_id: self.id,
                    lux,
                });
            }
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
            if let Some(node) = &dev.devnode
                && node.ends_with(name)
            {
                candidates.push(dev.clone());
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
        let Some(c) = self.get_connector(req.output) else {
            return Ok(());
        };
        let res = c.modify_state(&self.state, |s| {
            s.mode = backend::Mode {
                width: req.width,
                height: req.height,
                refresh_rate_millihz: req.refresh_rate_millihz,
            };
        });
        if let Err(e) = res {
            self.send_error(&format!("Could not modify connector mode: {}", ErrorFmt(e)));
        }
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
        let res = c.modify_state(&self.state, |s| s.enabled = req.enabled != 0);
        if let Err(e) = res {
            self.send_error(&format!("Could not en/disable connector: {}", ErrorFmt(e)));
        }
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
        c.connector.before_non_desktop_override_update(non_desktop);
        let res = c.modify_state(&self.state, |s| {
            s.non_desktop_override = non_desktop;
        });
        if let Err(e) = res {
            self.send_error(&format!("Could not change non-desktop override: {}", e));
        }
        Ok(())
    }

    fn set_vrr_mode(&self, req: SetVrrMode<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(mode) = VrrMode::from_config(ConfigVrrMode(req.mode)) else {
            return Err(JayRandrError::UnknownVrrMode(req.mode));
        };
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.set_vrr_mode(mode);
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

    fn set_tearing_mode(
        &self,
        req: SetTearingMode<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(mode) = TearingMode::from_config(ConfigTearingMode(req.mode)) else {
            return Err(JayRandrError::UnknownTearingMode(req.mode));
        };
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.set_tearing_mode(mode);
        return Ok(());
    }

    fn set_fb_format(&self, req: SetFbFormat<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(&format) = named_formats().get(req.format) else {
            return Err(JayRandrError::UnknownFormat(req.format.to_string()));
        };
        let Some(c) = self.get_connector(req.output) else {
            return Ok(());
        };
        let res = c.modify_state(&self.state, |s| s.format = format);
        if let Err(e) = res {
            self.send_error(&format!("Could not modify connector format: {}", e));
        }
        Ok(())
    }

    fn set_flip_margin(&self, req: SetFlipMargin<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(dev) = self.get_device(req.dev) else {
            return Ok(());
        };
        dev.dev.set_flip_margin(req.margin_ns);
        Ok(())
    }

    fn set_colors(&self, req: SetColors<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let cs = 'cs: {
            for cs in BackendColorSpace::variants() {
                if cs.name() == req.color_space {
                    break 'cs cs;
                }
            }
            return Err(JayRandrError::UnknownColorSpace(
                req.color_space.to_string(),
            ));
        };
        let tf = 'tf: {
            for tf in BackendTransferFunction::variants() {
                if tf.name() == req.transfer_function {
                    break 'tf tf;
                }
            }
            return Err(JayRandrError::UnknownTransferFunction(
                req.transfer_function.to_string(),
            ));
        };
        let Some(c) = self.get_connector(req.output) else {
            return Ok(());
        };
        let res = c.modify_state(&self.state, |s| {
            s.color_space = cs;
            s.transfer_function = tf;
        });
        if let Err(e) = res {
            self.send_error(&format!(
                "Could not modify connector colors: {}",
                ErrorFmt(e),
            ));
        }
        Ok(())
    }

    fn set_brightness(&self, req: SetBrightness<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.set_brightness(Some(req.lux));
        Ok(())
    }

    fn unset_brightness(
        &self,
        req: UnsetBrightness<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(c) = self.get_output_node(req.output) else {
            return Ok(());
        };
        c.set_brightness(None);
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
    #[error("Unknown tearing mode {0}")]
    UnknownTearingMode(u32),
    #[error("Unknown format {0}")]
    UnknownFormat(String),
    #[error("Unknown color space {0}")]
    UnknownColorSpace(String),
    #[error("Unknown transfer function {0}")]
    UnknownTransferFunction(String),
}
efrom!(JayRandrError, ClientError);
