use crate::async_engine::SpawnedFuture;
use crate::backend::transaction::BackendConnectorTransaction;
use crate::backend::transaction::BackendConnectorTransactionError;
use crate::backend::transaction::BackendConnectorTransactionType;
use crate::backend::transaction::BackendConnectorTransactionTypeDyn;
use crate::cmm::cmm_primaries::Primaries;
use crate::evdev::input_event_codes::InputEventCode;
use crate::fixed::Fixed;
use crate::format::Format;
use crate::gfx_api::FdSync;
use crate::gfx_api::GfxApi;
use crate::gfx_api::GfxFramebuffer;
use crate::ifs::wl_output::OutputId;
use crate::ifs::wl_seat::tablet::PadButtonState;
use crate::ifs::wl_seat::tablet::TabletInit;
use crate::ifs::wl_seat::tablet::TabletPadId;
use crate::ifs::wl_seat::tablet::TabletPadInit;
use crate::ifs::wl_seat::tablet::TabletRingEventSource;
use crate::ifs::wl_seat::tablet::TabletStripEventSource;
use crate::ifs::wl_seat::tablet::TabletToolChanges;
use crate::ifs::wl_seat::tablet::TabletToolId;
use crate::ifs::wl_seat::tablet::TabletToolInit;
use crate::ifs::wl_seat::tablet::ToolButtonState;
use crate::ifs::wl_seat::wl_pointer::CONTINUOUS;
use crate::ifs::wl_seat::wl_pointer::FINGER;
use crate::ifs::wl_seat::wl_pointer::HORIZONTAL_SCROLL;
use crate::ifs::wl_seat::wl_pointer::VERTICAL_SCROLL;
use crate::ifs::wl_seat::wl_pointer::WHEEL;
use crate::libinput::consts::ConfigScrollMethod;
use crate::libinput::consts::DeviceCapability;
use crate::libinput::consts::LIBINPUT_CONFIG_SCROLL_2FG;
use crate::libinput::consts::LIBINPUT_CONFIG_SCROLL_EDGE;
use crate::libinput::consts::LIBINPUT_CONFIG_SCROLL_NO_SCROLL;
use crate::libinput::consts::LIBINPUT_CONFIG_SCROLL_ON_BUTTON_DOWN;
use crate::utils::obj_and_id::ObjWithId;
use crate::utils::static_text::StaticText;
use crate::video::Modifier;
use crate::video::drm::ConnectorType;
use crate::video::drm::DRM_MODE_COLORIMETRY_BT2020_RGB;
use crate::video::drm::DRM_MODE_COLORIMETRY_DEFAULT;
use crate::video::drm::DrmConnector;
use crate::video::drm::DrmError;
use crate::video::drm::DrmVersion;
use crate::video::drm::HDMI_EOTF_SMPTE_ST2084;
use crate::video::drm::HDMI_EOTF_TRADITIONAL_GAMMA_SDR;
use jay_config::input::SwitchEvent;
use jay_proc::jay_hash;
use linearize::Linearize;
use linearize::StaticCopyMap;
use std::any::Any;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::Hash;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

pub mod transaction;

linear_ids!(ConnectorIds, ConnectorId);
linear_ids!(InputDeviceIds, InputDeviceId);
linear_ids!(DrmDeviceIds, DrmDeviceId);

pub trait Backend: Any {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>>;
    fn clear(&self) {
        // nothing
    }

    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn import_environment(&self) -> bool {
        false
    }

    fn supports_presentation_feedback(&self) -> bool {
        false
    }

    fn get_input_fds(&self) -> Vec<Rc<OwnedFd>> {
        vec![]
    }
}

#[jay_hash]
#[derive(Copy, Clone, Debug, Default, Eq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate_millihz: u32,
}

impl Mode {
    pub fn refresh_nsec(&self) -> u64 {
        match self.refresh_rate_millihz {
            0 => u64::MAX,
            n => 1_000_000_000_000 / (n as u64),
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{}@{}",
            self.width,
            self.height,
            self.refresh_rate_millihz as f64 / 1000.0,
        )
    }
}

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub modes: Option<Vec<Mode>>,
    pub output_id: Rc<OutputId>,
    pub width_mm: i32,
    pub height_mm: i32,
    pub non_desktop: bool,
    pub non_desktop_effective: bool,
    pub vrr_capable: bool,
    pub eotfs: Vec<BackendEotfs>,
    pub color_spaces: Vec<BackendColorSpace>,
    pub primaries: Primaries,
    pub luminance: Option<BackendLuminance>,
    pub state: BackendConnectorState,
}

#[derive(Copy, Clone, Debug)]
pub struct ConnectorKernelId {
    pub ty: ConnectorType,
    pub idx: u32,
}

impl Display for ConnectorKernelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.ty, self.idx)
    }
}

pub type ScanoutFormats = Rc<Vec<(&'static Format, Modifier)>>;

pub trait Connector: Any {
    fn id(&self) -> ConnectorId;
    fn kernel_id(&self) -> ConnectorKernelId;
    fn event(&self) -> Option<ConnectorEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn damage(&self);
    fn drm_dev(&self) -> Option<DrmDeviceId>;
    fn effectively_locked(&self) -> bool;
    fn state(&self) -> BackendConnectorState;
    fn drm_object_id(&self) -> Option<DrmConnector> {
        None
    }
    fn before_non_desktop_override_update(&self, overrd: Option<bool>) {
        let _ = overrd;
    }
    fn transaction_type(&self) -> Box<dyn BackendConnectorTransactionTypeDyn> {
        #[derive(Hash, Eq, PartialEq)]
        struct UnimplementedConnectorTransactionType;
        impl BackendConnectorTransactionType for UnimplementedConnectorTransactionType {}
        Box::new(UnimplementedConnectorTransactionType)
    }
    fn create_transaction(
        &self,
    ) -> Result<Box<dyn BackendConnectorTransaction>, BackendConnectorTransactionError> {
        Err(BackendConnectorTransactionError::TransactionsNotSupported(
            self.kernel_id(),
        ))
    }
    fn gamma_lut_size(&self) -> Option<u32> {
        None
    }
    fn name(&self) -> String {
        self.kernel_id().to_string()
    }
    fn scanout_formats(&self) -> Option<ScanoutFormats> {
        None
    }
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected(MonitorInfo),
    HardwareCursor(Option<Rc<dyn HardwareCursor>>),
    Disconnected,
    Removed,
    Unavailable,
    Available,
    State(BackendConnectorState),
    FormatsChanged(Rc<Vec<&'static Format>>),
}

pub trait HardwareCursorUpdate {
    fn set_enabled(&mut self, enabled: bool);
    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer>;
    fn set_position(&mut self, x: i32, y: i32);
    fn swap_buffer(&mut self, sync: Option<FdSync>);
    fn size(&self) -> (i32, i32);
}

pub trait HardwareCursor: Debug {
    fn damage(&self);
}

pub type TransformMatrix = [[f64; 2]; 2];

linear_ids!(InputDeviceGroupIds, InputDeviceGroupId, usize);

pub trait InputDevice {
    fn id(&self) -> InputDeviceId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<InputEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn grab(&self, grab: bool);
    fn has_capability(&self, cap: InputDeviceCapability) -> bool;
    fn left_handed(&self) -> Option<bool> {
        None
    }
    fn set_left_handed(&self, left_handed: bool);
    fn accel_profile(&self) -> Option<InputDeviceAccelProfile> {
        None
    }
    fn set_accel_profile(&self, profile: InputDeviceAccelProfile);
    fn accel_speed(&self) -> Option<f64> {
        None
    }
    fn set_accel_speed(&self, speed: f64);
    fn transform_matrix(&self) -> Option<TransformMatrix> {
        None
    }
    fn set_transform_matrix(&self, matrix: TransformMatrix);
    fn calibration_matrix(&self) -> Option<[[f32; 3]; 2]> {
        None
    }
    fn set_calibration_matrix(&self, m: [[f32; 3]; 2]) {
        let _ = m;
    }
    fn name(&self) -> Rc<String>;
    fn dev_t(&self) -> Option<c::dev_t> {
        None
    }
    fn tap_enabled(&self) -> Option<bool> {
        None
    }
    fn set_tap_enabled(&self, enabled: bool);
    fn drag_enabled(&self) -> Option<bool> {
        None
    }
    fn set_drag_enabled(&self, enabled: bool);
    fn drag_lock_enabled(&self) -> Option<bool> {
        None
    }
    fn set_drag_lock_enabled(&self, enabled: bool);
    fn natural_scrolling_enabled(&self) -> Option<bool> {
        None
    }
    fn set_natural_scrolling_enabled(&self, enabled: bool);
    fn click_method(&self) -> Option<InputDeviceClickMethod> {
        None
    }
    fn set_click_method(&self, method: InputDeviceClickMethod);
    fn middle_button_emulation_enabled(&self) -> Option<bool> {
        None
    }
    fn set_middle_button_emulation_enabled(&self, enabled: bool);
    fn tablet_info(&self) -> Option<Box<TabletInit>> {
        None
    }
    fn tablet_pad_info(&self) -> Option<Box<TabletPadInit>> {
        None
    }

    fn set_enabled_leds(&self, leds: Leds) {
        let _ = leds;
    }

    fn scroll_methods(&self) -> StaticCopyMap<InputDeviceScrollMethod, bool> {
        Default::default()
    }
    fn scroll_method(&self) -> Option<InputDeviceScrollMethod> {
        None
    }
    fn set_scroll_method(&self, method: InputDeviceScrollMethod);
    fn input_event_codes(&self) -> Vec<InputEventCode> {
        vec![]
    }
    fn scroll_button(&self) -> Option<InputEventCode> {
        None
    }
    fn set_scroll_button(&self, button: Option<InputEventCode>);
    fn scroll_button_lock(&self) -> Option<bool> {
        None
    }
    fn set_scroll_button_lock(&self, enabled: bool);
}

#[jay_hash]
#[derive(Debug, Copy, Clone, Eq, Linearize)]
pub enum InputDeviceCapability {
    Keyboard,
    Pointer,
    Touch,
    TabletTool,
    TabletPad,
    Gesture,
    Switch,
}

impl StaticText for InputDeviceCapability {
    fn text(&self) -> &'static str {
        match self {
            InputDeviceCapability::Keyboard => "keyboard",
            InputDeviceCapability::Pointer => "pointer",
            InputDeviceCapability::Touch => "touch",
            InputDeviceCapability::TabletTool => "tablet tool",
            InputDeviceCapability::TabletPad => "tablet pad",
            InputDeviceCapability::Gesture => "gesture",
            InputDeviceCapability::Switch => "switch",
        }
    }
}

impl InputDeviceCapability {
    pub fn to_libinput(self) -> DeviceCapability {
        use crate::libinput::consts::*;
        match self {
            InputDeviceCapability::Keyboard => LIBINPUT_DEVICE_CAP_KEYBOARD,
            InputDeviceCapability::Pointer => LIBINPUT_DEVICE_CAP_POINTER,
            InputDeviceCapability::Touch => LIBINPUT_DEVICE_CAP_TOUCH,
            InputDeviceCapability::TabletTool => LIBINPUT_DEVICE_CAP_TABLET_TOOL,
            InputDeviceCapability::TabletPad => LIBINPUT_DEVICE_CAP_TABLET_PAD,
            InputDeviceCapability::Gesture => LIBINPUT_DEVICE_CAP_GESTURE,
            InputDeviceCapability::Switch => LIBINPUT_DEVICE_CAP_SWITCH,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Linearize)]
pub enum InputDeviceAccelProfile {
    Flat,
    Adaptive,
}

impl StaticText for InputDeviceAccelProfile {
    fn text(&self) -> &'static str {
        match self {
            InputDeviceAccelProfile::Flat => "Flat",
            InputDeviceAccelProfile::Adaptive => "Adaptive",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Linearize)]
pub enum InputDeviceClickMethod {
    None,
    ButtonAreas,
    Clickfinger,
}

impl StaticText for InputDeviceClickMethod {
    fn text(&self) -> &'static str {
        match self {
            InputDeviceClickMethod::None => "none",
            InputDeviceClickMethod::ButtonAreas => "button-areas",
            InputDeviceClickMethod::Clickfinger => "clickfinger",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Linearize)]
pub enum InputDeviceScrollMethod {
    NoScroll,
    TwoFingers,
    Edge,
    OnButtonDown,
}

impl StaticText for InputDeviceScrollMethod {
    fn text(&self) -> &'static str {
        match self {
            InputDeviceScrollMethod::NoScroll => "No Scroll",
            InputDeviceScrollMethod::TwoFingers => "Two Fingers",
            InputDeviceScrollMethod::Edge => "Edge",
            InputDeviceScrollMethod::OnButtonDown => "On Button Down",
        }
    }
}

impl InputDeviceScrollMethod {
    pub fn to_libinput(self) -> ConfigScrollMethod {
        match self {
            InputDeviceScrollMethod::NoScroll => LIBINPUT_CONFIG_SCROLL_NO_SCROLL,
            InputDeviceScrollMethod::TwoFingers => LIBINPUT_CONFIG_SCROLL_2FG,
            InputDeviceScrollMethod::Edge => LIBINPUT_CONFIG_SCROLL_EDGE,
            InputDeviceScrollMethod::OnButtonDown => LIBINPUT_CONFIG_SCROLL_ON_BUTTON_DOWN,
        }
    }

    pub fn from_libinput(v: ConfigScrollMethod) -> Option<Self> {
        let m = match v {
            LIBINPUT_CONFIG_SCROLL_NO_SCROLL => InputDeviceScrollMethod::NoScroll,
            LIBINPUT_CONFIG_SCROLL_2FG => InputDeviceScrollMethod::TwoFingers,
            LIBINPUT_CONFIG_SCROLL_EDGE => InputDeviceScrollMethod::Edge,
            LIBINPUT_CONFIG_SCROLL_ON_BUTTON_DOWN => InputDeviceScrollMethod::OnButtonDown,
            _ => return None,
        };
        Some(m)
    }
}

pub enum BackendEvent {
    NewDrmDevice(Rc<dyn BackendDrmDevice>),
    NewConnector(Rc<dyn Connector>),
    NewInputDevice(Rc<dyn InputDevice>),
    DevicesEnumerated,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum KeyState {
    Released,
    Pressed,
    Repeated,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ButtonState {
    Released,
    Pressed,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Linearize)]
pub enum ScrollAxis {
    Horizontal = HORIZONTAL_SCROLL as _,
    Vertical = VERTICAL_SCROLL as _,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AxisSource {
    Wheel = WHEEL as _,
    Finger = FINGER as _,
    Continuous = CONTINUOUS as _,
}

pub const AXIS_120: i32 = 120;

bitflags! {
    Leds: u32;
        LED_NUM_LOCK,
        LED_CAPS_LOCK,
        LED_SCROLL_LOCK,
        LED_COMPOSE,
        LED_KANA,
}

#[derive(Debug)]
pub enum InputEvent {
    Key {
        time_usec: u64,
        key: u32,
        state: KeyState,
    },
    ConnectorPosition {
        time_usec: u64,
        connector: ConnectorId,
        x: Fixed,
        y: Fixed,
    },
    Motion {
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    },
    MotionAbsolute {
        time_usec: u64,
        x_normed: f32,
        y_normed: f32,
    },
    Button {
        time_usec: u64,
        button: u32,
        state: ButtonState,
    },

    AxisPx {
        dist: f64,
        axis: ScrollAxis,
        inverted: bool,
    },
    AxisSource {
        source: AxisSource,
    },
    AxisStop {
        axis: ScrollAxis,
    },
    Axis120 {
        dist: i32,
        axis: ScrollAxis,
        inverted: bool,
    },
    AxisFrame {
        time_usec: u64,
    },
    SwipeBegin {
        time_usec: u64,
        finger_count: u32,
    },
    SwipeUpdate {
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    },
    SwipeEnd {
        time_usec: u64,
        cancelled: bool,
    },
    PinchBegin {
        time_usec: u64,
        finger_count: u32,
    },
    PinchUpdate {
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
        scale: Fixed,
        rotation: Fixed,
    },
    PinchEnd {
        time_usec: u64,
        cancelled: bool,
    },
    HoldBegin {
        time_usec: u64,
        finger_count: u32,
    },
    HoldEnd {
        time_usec: u64,
        cancelled: bool,
    },

    SwitchEvent {
        time_usec: u64,
        event: SwitchEvent,
    },

    TabletToolAdded {
        time_usec: u64,
        init: Box<TabletToolInit>,
    },
    TabletToolChanged {
        time_usec: u64,
        id: TabletToolId,
        changes: Box<TabletToolChanges>,
    },
    TabletToolButton {
        time_usec: u64,
        id: TabletToolId,
        button: u32,
        state: ToolButtonState,
    },
    TabletToolRemoved {
        time_usec: u64,
        id: TabletToolId,
    },

    TabletPadButton {
        time_usec: u64,
        id: TabletPadId,
        button: u32,
        state: PadButtonState,
    },
    TabletPadModeSwitch {
        time_usec: u64,
        pad: TabletPadId,
        group: u32,
        mode: u32,
    },
    TabletPadRing {
        time_usec: u64,
        pad: TabletPadId,
        ring: u32,
        source: Option<TabletRingEventSource>,
        angle: Option<f64>,
    },
    TabletPadStrip {
        time_usec: u64,
        pad: TabletPadId,
        strip: u32,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
    },
    TabletPadDial {
        time_usec: u64,
        pad: TabletPadId,
        dial: u32,
        value120: i32,
    },
    TouchDown {
        time_usec: u64,
        id: i32,
        x_normed: Fixed,
        y_normed: Fixed,
    },
    TouchUp {
        time_usec: u64,
        id: i32,
    },
    TouchMotion {
        time_usec: u64,
        id: i32,
        x_normed: Fixed,
        y_normed: Fixed,
    },
    TouchCancel {
        time_usec: u64,
        id: i32,
    },
    TouchFrame {
        time_usec: u64,
    },
}

pub enum DrmEvent {
    #[expect(dead_code)]
    Removed,
    GfxApiChanged,
}

pub trait BackendDrmDevice {
    fn id(&self) -> DrmDeviceId;
    fn event(&self) -> Option<DrmEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn dev_t(&self) -> c::dev_t;
    fn make_render_device(&self);
    fn set_gfx_api(&self, api: GfxApi);
    fn gfx_api(&self) -> GfxApi;
    fn version(&self) -> Result<DrmVersion, DrmError>;
    fn set_direct_scanout_enabled(&self, enabled: bool);
    fn is_render_device(&self) -> bool;
    fn direct_scanout_enabled(&self) -> bool {
        false
    }
    fn create_lease(
        self: Rc<Self>,
        lessee: Rc<dyn BackendDrmLessee>,
        connector_ids: &[ConnectorId],
    ) {
        let _ = lessee;
        let _ = connector_ids;
    }
    fn set_flip_margin(&self, margin: u64) {
        let _ = margin;
    }
    fn flip_margin(&self) -> Option<u64> {
        None
    }
}

pub trait BackendDrmLease {
    fn fd(&self) -> &Rc<OwnedFd>;
}

pub trait BackendDrmLessee {
    fn created(&self, lease: Rc<dyn BackendDrmLease>);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Linearize)]
pub enum BackendEotfs {
    #[default]
    Default,
    Pq,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Linearize)]
pub enum BackendColorSpace {
    #[default]
    Default,
    Bt2020,
}

#[derive(Copy, Clone, Debug)]
pub struct BackendLuminance {
    pub min: f64,
    pub max: f64,
    pub max_fall: f64,
}

impl BackendEotfs {
    pub fn to_drm(self) -> u8 {
        match self {
            BackendEotfs::Default => HDMI_EOTF_TRADITIONAL_GAMMA_SDR,
            BackendEotfs::Pq => HDMI_EOTF_SMPTE_ST2084,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            BackendEotfs::Default => "default",
            BackendEotfs::Pq => "pq",
        }
    }
}

impl BackendColorSpace {
    pub fn to_drm(self) -> u64 {
        match self {
            BackendColorSpace::Default => DRM_MODE_COLORIMETRY_DEFAULT,
            BackendColorSpace::Bt2020 => DRM_MODE_COLORIMETRY_BT2020_RGB,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            BackendColorSpace::Default => "default",
            BackendColorSpace::Bt2020 => "bt2020",
        }
    }
}

// kernel: struct drm_color_lut
pub type BackendGammaLutElement = [u16; 4];
pub type BackendGammaLutId = [u8; 32];

#[derive(Debug, Eq)]
pub struct BackendGammaLut {
    id: BackendGammaLutId,
    pub gamma_lut: Vec<BackendGammaLutElement>,
}

impl BackendGammaLut {
    pub fn new(mut gamma_lut: Vec<BackendGammaLutElement>) -> Self {
        for element in &mut gamma_lut {
            element[3] = 0;
        }
        let gamma_lut_bytes = uapi::as_bytes(&gamma_lut as &[_]);
        let id = *blake3::hash(gamma_lut_bytes).as_bytes();
        Self { id, gamma_lut }
    }
}

impl ObjWithId for Rc<BackendGammaLut> {
    type Id = BackendGammaLutId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl PartialEq for BackendGammaLut {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

linear_ids!(
    BackendConnectorStateSerials,
    BackendConnectorStateSerial,
    u64
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendConnectorState {
    pub serial: BackendConnectorStateSerial,
    pub enabled: bool,
    pub active: bool,
    pub mode: Mode,
    pub non_desktop_override: Option<bool>,
    pub vrr: bool,
    pub tearing: bool,
    pub format: &'static Format,
    pub color_space: BackendColorSpace,
    pub eotf: BackendEotfs,
    pub gamma_lut: Option<Rc<BackendGammaLut>>,
}
