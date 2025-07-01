//! Tools for configuring graphics cards and monitors.

use {
    crate::{
        _private::WireMode,
        PciId, Workspace,
        video::connector_type::{
            CON_9PIN_DIN, CON_COMPONENT, CON_COMPOSITE, CON_DISPLAY_PORT, CON_DPI, CON_DSI,
            CON_DVIA, CON_DVID, CON_DVII, CON_EDP, CON_EMBEDDED_WINDOW, CON_HDMIA, CON_HDMIB,
            CON_LVDS, CON_SPI, CON_SVIDEO, CON_TV, CON_UNKNOWN, CON_USB, CON_VGA, CON_VIRTUAL,
            CON_WRITEBACK, ConnectorType,
        },
    },
    serde::{Deserialize, Serialize},
    std::{str::FromStr, time::Duration},
};

/// The mode of a connector.
///
/// Currently a mode consists of three properties:
///
/// - width in pixels
/// - height in pixels
/// - refresh rate in mhz.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Mode {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) refresh_millihz: u32,
}

impl Mode {
    /// Returns the width of the mode.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Returns the height of the mode.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Returns the refresh rate of the mode in mhz.
    ///
    /// For a 60hz monitor, this function would return 60_000.
    pub fn refresh_rate(&self) -> u32 {
        self.refresh_millihz
    }

    pub(crate) fn zeroed() -> Self {
        Self {
            width: 0,
            height: 0,
            refresh_millihz: 0,
        }
    }
}

/// A connector that is potentially connected to an output device.
///
/// A connector is the part that sticks out of your graphics card. A graphics card usually
/// has many connectors but one few of them are actually connected to a monitor.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Connector(pub u64);

impl Connector {
    /// Returns whether this connector existed at the time `get_connector` was called.
    ///
    /// This only implies existence at the time `get_connector` was called. Even if this
    /// function returns true, the connector might since have disappeared.
    pub fn exists(self) -> bool {
        self.0 != 0
    }

    /// Returns whether the connector is connected to an output device.
    pub fn connected(self) -> bool {
        if !self.exists() {
            return false;
        }
        get!(false).connector_connected(self)
    }

    /// Returns the scale of the currently connected monitor.
    pub fn scale(self) -> f64 {
        if !self.exists() {
            return 1.0;
        }
        get!(1.0).connector_get_scale(self)
    }

    /// Sets the scale to use for the currently connected monitor.
    pub fn set_scale(self, scale: f64) {
        if !self.exists() {
            return;
        }
        log::info!("setting scale to {}", scale);
        get!().connector_set_scale(self, scale);
    }

    /// Returns the connector type.
    pub fn ty(self) -> ConnectorType {
        if !self.exists() {
            return CON_UNKNOWN;
        }
        get!(CON_UNKNOWN).connector_type(self)
    }

    /// Returns the current mode of the connector.
    pub fn mode(self) -> Mode {
        if !self.exists() {
            return Mode::zeroed();
        }
        get!(Mode::zeroed()).connector_mode(self)
    }

    /// Tries to set the mode of the connector.
    ///
    /// If the refresh rate is not specified, tries to use the first mode with the given
    /// width and height.
    ///
    /// The default mode is the first mode advertised by the connector. This is usually
    /// the native mode.
    pub fn set_mode(self, width: i32, height: i32, refresh_millihz: Option<u32>) {
        if !self.exists() {
            log::warn!("set_mode called on a connector that does not exist");
            return;
        }
        let refresh_millihz = match refresh_millihz {
            Some(r) => r,
            _ => match self
                .modes()
                .iter()
                .find(|m| m.width == width && m.height == height)
            {
                Some(m) => m.refresh_millihz,
                _ => {
                    log::warn!("Could not find any mode with width {width} and height {height}");
                    return;
                }
            },
        };
        get!().connector_set_mode(
            self,
            WireMode {
                width,
                height,
                refresh_millihz,
            },
        )
    }

    /// Returns the available modes of the connector.
    pub fn modes(self) -> Vec<Mode> {
        if !self.exists() {
            return Vec::new();
        }
        get!(Vec::new()).connector_modes(self)
    }

    /// Returns the logical width of the connector.
    ///
    /// The returned value will be different from `mode().width()` if the scale is not 1.
    pub fn width(self) -> i32 {
        get!().connector_size(self).0
    }

    /// Returns the logical height of the connector.
    ///
    /// The returned value will be different from `mode().height()` if the scale is not 1.
    pub fn height(self) -> i32 {
        get!().connector_size(self).1
    }

    /// Returns the refresh rate in mhz of the current mode of the connector.
    ///
    /// This is a shortcut for `mode().refresh_rate()`.
    pub fn refresh_rate(self) -> u32 {
        self.mode().refresh_millihz
    }

    /// Retrieves the position of the output in the global compositor space.
    pub fn position(self) -> (i32, i32) {
        if !self.connected() {
            return (0, 0);
        }
        get!().connector_get_position(self)
    }

    /// Sets the position of the connector in the global compositor space.
    ///
    /// `x` and `y` must be non-negative and must not exceed a currently unspecified limit.
    /// Any reasonable values for `x` and `y` should work.
    ///
    /// This function allows the connector to overlap with other connectors, however, such
    /// configurations are not supported and might result in unexpected behavior.
    pub fn set_position(self, x: i32, y: i32) {
        if !self.exists() {
            log::warn!("set_position called on a connector that does not exist");
            return;
        }
        get!().connector_set_position(self, x, y);
    }

    /// Enables or disables the connector.
    ///
    /// By default, all connectors are enabled.
    pub fn set_enabled(self, enabled: bool) {
        if !self.exists() {
            log::warn!("set_enabled called on a connector that does not exist");
            return;
        }
        get!().connector_set_enabled(self, enabled);
    }

    /// Sets the transformation to apply to the content of this connector.
    pub fn set_transform(self, transform: Transform) {
        if !self.exists() {
            log::warn!("set_transform called on a connector that does not exist");
            return;
        }
        get!().connector_set_transform(self, transform);
    }

    pub fn name(self) -> String {
        if !self.exists() {
            return String::new();
        }
        get!(String::new()).connector_get_name(self)
    }

    pub fn model(self) -> String {
        if !self.exists() {
            return String::new();
        }
        get!(String::new()).connector_get_model(self)
    }

    pub fn manufacturer(self) -> String {
        if !self.exists() {
            return String::new();
        }
        get!(String::new()).connector_get_manufacturer(self)
    }

    pub fn serial_number(self) -> String {
        if !self.exists() {
            return String::new();
        }
        get!(String::new()).connector_get_serial_number(self)
    }

    /// Sets the VRR mode.
    pub fn set_vrr_mode(self, mode: VrrMode) {
        get!().set_vrr_mode(Some(self), mode)
    }

    /// Sets the VRR cursor refresh rate.
    ///
    /// Limits the rate at which cursors are updated on screen when VRR is active.
    ///
    /// Setting this to infinity disables the limiter.
    pub fn set_vrr_cursor_hz(self, hz: f64) {
        get!().set_vrr_cursor_hz(Some(self), hz)
    }

    /// Sets the tearing mode.
    pub fn set_tearing_mode(self, mode: TearingMode) {
        get!().set_tearing_mode(Some(self), mode)
    }

    /// Sets the format to use for framebuffers.
    pub fn set_format(self, format: Format) {
        get!().connector_set_format(self, format);
    }

    /// Sets the color space and transfer function of the connector.
    ///
    /// By default, the default values are used which usually means sRGB color space with
    /// sRGB transfer function.
    ///
    /// If the output supports it, HDR10 can be enabled by setting the color space to
    /// BT.2020 and the transfer function to PQ.
    ///
    /// Note that some displays might ignore incompatible settings.
    pub fn set_colors(self, color_space: ColorSpace, transfer_function: TransferFunction) {
        get!().connector_set_colors(self, color_space, transfer_function);
    }

    /// Sets the brightness of the output.
    ///
    /// By default or when `brightness` is `None`, the brightness depends on the
    /// transfer function:
    ///
    /// - [`TransferFunction::DEFAULT`]: The maximum brightness of the output.
    /// - [`TransferFunction::PQ`]: 203 cd/m^2.
    ///
    /// This should only be used with the PQ transfer function. If the default transfer
    /// function is used, you should instead calibrate the hardware directly.
    ///
    /// This has no effect unless the vulkan renderer is used.
    pub fn set_brightness(self, brightness: Option<f64>) {
        get!().connector_set_brightness(self, brightness);
    }

    /// Get the currently visible/active workspace.
    ///
    /// If this connector is not connected, or is there no active workspace, returns a
    /// workspace whose `exists()` returns false.
    pub fn active_workspace(self) -> Workspace {
        get!(Workspace(0)).get_connector_active_workspace(self)
    }

    /// Get all workspaces on this connector.
    ///
    /// If this connector is not connected, returns an empty list.
    pub fn workspaces(self) -> Vec<Workspace> {
        get!().get_connector_workspaces(self)
    }
}

/// Returns all available DRM devices.
pub fn drm_devices() -> Vec<DrmDevice> {
    get!().drm_devices()
}

/// Sets the callback to be called when a new DRM device appears.
pub fn on_new_drm_device<F: FnMut(DrmDevice) + 'static>(f: F) {
    get!().on_new_drm_device(f)
}

/// Sets the callback to be called when a DRM device is removed.
pub fn on_drm_device_removed<F: FnMut(DrmDevice) + 'static>(f: F) {
    get!().on_del_drm_device(f)
}

/// Sets the callback to be called when a new connector appears.
pub fn on_new_connector<F: FnMut(Connector) + 'static>(f: F) {
    get!().on_new_connector(f)
}

/// Sets the callback to be called when a connector becomes connected to an output device.
pub fn on_connector_connected<F: FnMut(Connector) + 'static>(f: F) {
    get!().on_connector_connected(f)
}

/// Sets the callback to be called when a connector is disconnected from an output device.
pub fn on_connector_disconnected<F: FnMut(Connector) + 'static>(f: F) {
    get!().on_connector_disconnected(f)
}

/// Sets the callback to be called when the graphics of the compositor have been initialized.
///
/// This callback is only invoked once during the lifetime of the compositor. This is a good place
/// to auto start graphical applications.
pub fn on_graphics_initialized<F: FnOnce() + 'static>(f: F) {
    get!().on_graphics_initialized(f)
}

pub fn connectors() -> Vec<Connector> {
    get!().connectors(None)
}

/// Returns the connector with the given id.
///
/// The linux kernel identifies connectors by a (type, idx) tuple, e.g., `DP-0`.
/// If the connector does not exist at the time this function is called, a sentinel value is
/// returned. This can be checked by calling `exists()` on the returned connector.
///
/// The `id` argument can either be an explicit tuple, e.g. `(CON_DISPLAY_PORT, 0)`, or a string
/// that can be parsed to such a tuple, e.g. `"DP-0"`.
///
/// The following string prefixes exist:
///
/// - `DP`
/// - `eDP`
/// - `HDMI-A`
/// - `HDMI-B`
/// - `EmbeddedWindow` - this is an implementation detail of the compositor and used if it
///   runs as an embedded application.
/// - `VGA`
/// - `DVI-I`
/// - `DVI-D`
/// - `DVI-A`
/// - `Composite`
/// - `SVIDEO`
/// - `LVDS`
/// - `Component`
/// - `DIN`
/// - `TV`
/// - `Virtual`
/// - `DSI`
/// - `DPI`
/// - `Writeback`
/// - `SPI`
/// - `USB`
pub fn get_connector(id: impl ToConnectorId) -> Connector {
    let (ty, idx) = match id.to_connector_id() {
        Ok(id) => id,
        Err(e) => {
            log::error!("{}", e);
            return Connector(0);
        }
    };
    get!(Connector(0)).get_connector(ty, idx)
}

/// A type that can be converted to a `(ConnectorType, idx)` tuple.
pub trait ToConnectorId {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String>;
}

impl ToConnectorId for (ConnectorType, u32) {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String> {
        Ok(*self)
    }
}

impl ToConnectorId for &'_ str {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String> {
        let pairs = [
            ("DP-", CON_DISPLAY_PORT),
            ("eDP-", CON_EDP),
            ("HDMI-A-", CON_HDMIA),
            ("HDMI-B-", CON_HDMIB),
            ("EmbeddedWindow-", CON_EMBEDDED_WINDOW),
            ("VGA-", CON_VGA),
            ("DVI-I-", CON_DVII),
            ("DVI-D-", CON_DVID),
            ("DVI-A-", CON_DVIA),
            ("Composite-", CON_COMPOSITE),
            ("SVIDEO-", CON_SVIDEO),
            ("LVDS-", CON_LVDS),
            ("Component-", CON_COMPONENT),
            ("DIN-", CON_9PIN_DIN),
            ("TV-", CON_TV),
            ("Virtual-", CON_VIRTUAL),
            ("DSI-", CON_DSI),
            ("DPI-", CON_DPI),
            ("Writeback-", CON_WRITEBACK),
            ("SPI-", CON_SPI),
            ("USB-", CON_USB),
        ];
        for (prefix, ty) in pairs {
            if let Some(suffix) = self.strip_prefix(prefix)
                && let Ok(idx) = u32::from_str(suffix)
            {
                return Ok((ty, idx));
            }
        }
        Err(format!("`{}` is not a valid connector identifier", self))
    }
}

/// Module containing all known connector types.
pub mod connector_type {
    use serde::{Deserialize, Serialize};

    /// The type of a connector.
    #[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
    pub struct ConnectorType(pub u32);

    pub const CON_UNKNOWN: ConnectorType = ConnectorType(0);
    pub const CON_VGA: ConnectorType = ConnectorType(1);
    pub const CON_DVII: ConnectorType = ConnectorType(2);
    pub const CON_DVID: ConnectorType = ConnectorType(3);
    pub const CON_DVIA: ConnectorType = ConnectorType(4);
    pub const CON_COMPOSITE: ConnectorType = ConnectorType(5);
    pub const CON_SVIDEO: ConnectorType = ConnectorType(6);
    pub const CON_LVDS: ConnectorType = ConnectorType(7);
    pub const CON_COMPONENT: ConnectorType = ConnectorType(8);
    pub const CON_9PIN_DIN: ConnectorType = ConnectorType(9);
    pub const CON_DISPLAY_PORT: ConnectorType = ConnectorType(10);
    pub const CON_HDMIA: ConnectorType = ConnectorType(11);
    pub const CON_HDMIB: ConnectorType = ConnectorType(12);
    pub const CON_TV: ConnectorType = ConnectorType(13);
    pub const CON_EDP: ConnectorType = ConnectorType(14);
    pub const CON_VIRTUAL: ConnectorType = ConnectorType(15);
    pub const CON_DSI: ConnectorType = ConnectorType(16);
    pub const CON_DPI: ConnectorType = ConnectorType(17);
    pub const CON_WRITEBACK: ConnectorType = ConnectorType(18);
    pub const CON_SPI: ConnectorType = ConnectorType(19);
    pub const CON_USB: ConnectorType = ConnectorType(20);
    pub const CON_EMBEDDED_WINDOW: ConnectorType = ConnectorType(u32::MAX);
}

/// A *Direct Rendering Manager* (DRM) device.
///
/// It's easiest to think of a DRM device as a graphics card.
/// There are also DRM devices that are emulated in software but you are unlikely to encounter
/// those accidentally.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct DrmDevice(pub u64);

impl DrmDevice {
    /// Returns the connectors of this device.
    pub fn connectors(self) -> Vec<Connector> {
        get!().connectors(Some(self))
    }

    /// Returns the devnode of this device.
    ///
    /// E.g. `/dev/dri/card0`.
    pub fn devnode(self) -> String {
        get!().drm_device_devnode(self)
    }

    /// Returns the syspath of this device.
    ///
    /// E.g. `/sys/devices/pci0000:00/0000:00:03.1/0000:07:00.0`.
    pub fn syspath(self) -> String {
        get!().drm_device_syspath(self)
    }

    /// Returns the vendor of this device.
    ///
    /// E.g. `Advanced Micro Devices, Inc. [AMD/ATI]`.
    pub fn vendor(self) -> String {
        get!().drm_device_vendor(self)
    }

    /// Returns the model of this device.
    ///
    /// E.g. `Ellesmere [Radeon RX 470/480/570/570X/580/580X/590] (Radeon RX 570 Armor 8G OC)`.
    pub fn model(self) -> String {
        get!().drm_device_model(self)
    }

    /// Returns the PIC ID of this device.
    ///
    /// E.g. `1002:67DF`.
    pub fn pci_id(self) -> PciId {
        get!().drm_device_pci_id(self)
    }

    /// Makes this device the render device.
    pub fn make_render_device(self) {
        get!().make_render_device(self);
    }

    /// Sets the preferred graphics API for this device.
    ///
    /// If the API cannot be used, the compositor will try other APIs.
    pub fn set_gfx_api(self, gfx_api: GfxApi) {
        get!().set_gfx_api(Some(self), gfx_api);
    }

    /// Enables or disables direct scanout of client surfaces for this device.
    pub fn set_direct_scanout_enabled(self, enabled: bool) {
        get!().set_direct_scanout_enabled(Some(self), enabled);
    }

    /// Sets the flip margin of this device.
    ///
    /// This is duration between the compositor initiating a page flip and the output's
    /// vblank event. This determines the minimum input latency. The default is 1.5 ms.
    ///
    /// Note that if the margin is too small, the compositor will dynamically increase it.
    pub fn set_flip_margin(self, margin: Duration) {
        get!().set_flip_margin(self, margin);
    }
}

/// A graphics API.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GfxApi {
    OpenGl,
    Vulkan,
}

/// Sets the default graphics API.
///
/// If the API cannot be used, the compositor will try other APIs.
///
/// This setting can be overwritten per-device with [DrmDevice::set_gfx_api].
///
/// This call has no effect on devices that have already been initialized.
pub fn set_gfx_api(gfx_api: GfxApi) {
    get!().set_gfx_api(None, gfx_api);
}

/// Enables or disables direct scanout of client surfaces.
///
/// The default is `true`.
///
/// This setting can be overwritten per-device with [DrmDevice::set_direct_scanout_enabled].
pub fn set_direct_scanout_enabled(enabled: bool) {
    get!().set_direct_scanout_enabled(None, enabled);
}

/// A transformation.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub enum Transform {
    /// No transformation.
    #[default]
    None,
    /// Rotate 90 degrees counter-clockwise.
    Rotate90,
    /// Rotate 180 degrees counter-clockwise.
    Rotate180,
    /// Rotate 270 degrees counter-clockwise.
    Rotate270,
    /// Flip around the vertical axis.
    Flip,
    /// Flip around the vertical axis, then rotate 90 degrees counter-clockwise.
    FlipRotate90,
    /// Flip around the vertical axis, then rotate 180 degrees counter-clockwise.
    FlipRotate180,
    /// Flip around the vertical axis, then rotate 270 degrees counter-clockwise.
    FlipRotate270,
}

/// The VRR mode of a connector.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct VrrMode(pub u32);

impl VrrMode {
    /// VRR is never enabled.
    pub const NEVER: Self = Self(0);
    /// VRR is always enabled.
    pub const ALWAYS: Self = Self(1);
    /// VRR is enabled when one or more applications are displayed fullscreen.
    pub const VARIANT_1: Self = Self(2);
    /// VRR is enabled when a single application is displayed fullscreen.
    pub const VARIANT_2: Self = Self(3);
    /// VRR is enabled when a single game or video is displayed fullscreen.
    pub const VARIANT_3: Self = Self(4);
}

/// Sets the default VRR mode.
///
/// This setting can be overwritten on a per-connector basis with [Connector::set_vrr_mode].
pub fn set_vrr_mode(mode: VrrMode) {
    get!().set_vrr_mode(None, mode)
}

/// Sets the VRR cursor refresh rate.
///
/// Limits the rate at which cursors are updated on screen when VRR is active.
///
/// Setting this to infinity disables the limiter.
///
/// This setting can be overwritten on a per-connector basis with [Connector::set_vrr_cursor_hz].
pub fn set_vrr_cursor_hz(hz: f64) {
    get!().set_vrr_cursor_hz(None, hz)
}

/// The tearing mode of a connector.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct TearingMode(pub u32);

impl TearingMode {
    /// Tearing is never enabled.
    pub const NEVER: Self = Self(0);
    /// Tearing is always enabled.
    pub const ALWAYS: Self = Self(1);
    /// Tearing is enabled when one or more applications are displayed fullscreen.
    pub const VARIANT_1: Self = Self(2);
    /// Tearing is enabled when a single application is displayed fullscreen.
    pub const VARIANT_2: Self = Self(3);
    /// Tearing is enabled when a single application is displayed fullscreen and the
    /// application has requested tearing.
    ///
    /// This is the default.
    pub const VARIANT_3: Self = Self(4);
}

/// Sets the default tearing mode.
///
/// This setting can be overwritten on a per-connector basis with [Connector::set_tearing_mode].
pub fn set_tearing_mode(mode: TearingMode) {
    get!().set_tearing_mode(None, mode)
}

/// A graphics format.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Format(pub u32);

impl Format {
    pub const ARGB8888: Self = Self(0);
    pub const XRGB8888: Self = Self(1);
    pub const ABGR8888: Self = Self(2);
    pub const XBGR8888: Self = Self(3);
    pub const R8: Self = Self(4);
    pub const GR88: Self = Self(5);
    pub const RGB888: Self = Self(6);
    pub const BGR888: Self = Self(7);
    pub const RGBA4444: Self = Self(8);
    pub const RGBX4444: Self = Self(9);
    pub const BGRA4444: Self = Self(10);
    pub const BGRX4444: Self = Self(11);
    pub const RGB565: Self = Self(12);
    pub const BGR565: Self = Self(13);
    pub const RGBA5551: Self = Self(14);
    pub const RGBX5551: Self = Self(15);
    pub const BGRA5551: Self = Self(16);
    pub const BGRX5551: Self = Self(17);
    pub const ARGB1555: Self = Self(18);
    pub const XRGB1555: Self = Self(19);
    pub const ARGB2101010: Self = Self(20);
    pub const XRGB2101010: Self = Self(21);
    pub const ABGR2101010: Self = Self(22);
    pub const XBGR2101010: Self = Self(23);
    pub const ABGR16161616: Self = Self(24);
    pub const XBGR16161616: Self = Self(25);
    pub const ABGR16161616F: Self = Self(26);
    pub const XBGR16161616F: Self = Self(27);
}

/// A color space.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ColorSpace(pub u32);

impl ColorSpace {
    /// The default color space (usually sRGB).
    pub const DEFAULT: Self = Self(0);
    /// The BT.2020 color space.
    pub const BT2020: Self = Self(1);
}

/// A transfer function.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransferFunction(pub u32);

impl TransferFunction {
    /// The default transfer function (usually sRGB).
    pub const DEFAULT: Self = Self(0);
    /// The PQ transfer function.
    pub const PQ: Self = Self(1);
}
