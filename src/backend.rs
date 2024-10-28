use {
    crate::{
        async_engine::SpawnedFuture,
        drm_feedback::DrmFeedback,
        fixed::Fixed,
        format::Format,
        gfx_api::{GfxFramebuffer, SyncFile},
        ifs::{
            wl_output::OutputId,
            wl_seat::{
                tablet::{
                    PadButtonState, TabletInit, TabletPadId, TabletPadInit, TabletRingEventSource,
                    TabletStripEventSource, TabletToolChanges, TabletToolId, TabletToolInit,
                    ToolButtonState,
                },
                wl_pointer::{CONTINUOUS, FINGER, HORIZONTAL_SCROLL, VERTICAL_SCROLL, WHEEL},
            },
        },
        libinput::consts::DeviceCapability,
        video::drm::{ConnectorType, DrmConnector, DrmError, DrmVersion},
    },
    jay_config::{input::SwitchEvent, video::GfxApi},
    std::{
        any::Any,
        error::Error,
        fmt::{Debug, Display, Formatter},
        rc::Rc,
    },
    uapi::{c, OwnedFd},
};

linear_ids!(ConnectorIds, ConnectorId);
linear_ids!(InputDeviceIds, InputDeviceId);
linear_ids!(DrmDeviceIds, DrmDeviceId);

pub trait Backend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>>;
    fn clear(&self) {
        // nothing
    }
    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    fn into_any(self: Rc<Self>) -> Rc<dyn Any>;

    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn set_idle(&self, idle: bool) {
        let _ = idle;
    }

    fn import_environment(&self) -> bool {
        false
    }

    fn supports_presentation_feedback(&self) -> bool {
        false
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
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
}

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub modes: Vec<Mode>,
    pub output_id: Rc<OutputId>,
    pub initial_mode: Mode,
    pub width_mm: i32,
    pub height_mm: i32,
    pub non_desktop: bool,
    pub vrr_capable: bool,
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

pub trait Connector {
    fn id(&self) -> ConnectorId;
    fn kernel_id(&self) -> ConnectorKernelId;
    fn event(&self) -> Option<ConnectorEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn damage(&self);
    fn drm_dev(&self) -> Option<DrmDeviceId>;
    fn enabled(&self) -> bool {
        true
    }
    fn set_enabled(&self, enabled: bool) {
        let _ = enabled;
    }
    fn drm_feedback(&self) -> Option<Rc<DrmFeedback>> {
        None
    }
    fn set_mode(&self, mode: Mode);
    fn set_non_desktop_override(&self, non_desktop: Option<bool>) {
        let _ = non_desktop;
    }
    fn drm_object_id(&self) -> Option<DrmConnector> {
        None
    }
    fn set_vrr_enabled(&self, enabled: bool) {
        let _ = enabled;
    }
    fn set_tearing_enabled(&self, enabled: bool) {
        let _ = enabled;
    }
    fn set_fb_format(&self, format: &'static Format) {
        let _ = format;
    }
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected(MonitorInfo),
    HardwareCursor(Option<Rc<dyn HardwareCursor>>),
    Disconnected,
    Removed,
    ModeChanged(Mode),
    Unavailable,
    Available,
    VrrChanged(bool),
    FormatsChanged(Rc<Vec<&'static Format>>, &'static Format),
}

pub trait HardwareCursorUpdate {
    fn set_enabled(&mut self, enabled: bool);
    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer>;
    fn set_position(&mut self, x: i32, y: i32);
    fn swap_buffer(&mut self);
    fn set_sync_file(&mut self, sync_file: Option<SyncFile>);
    fn size(&self) -> (i32, i32);
}

pub trait HardwareCursor: Debug {
    fn damage(&self);
    fn passive_damage(&self);
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
    fn tablet_info(&self) -> Option<Box<TabletInit>> {
        None
    }
    fn tablet_pad_info(&self) -> Option<Box<TabletPadInit>> {
        None
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum InputDeviceCapability {
    Keyboard,
    Pointer,
    Touch,
    TabletTool,
    TabletPad,
    Gesture,
    Switch,
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

#[derive(Debug, Copy, Clone)]
pub enum InputDeviceAccelProfile {
    Flat,
    Adaptive,
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
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
    Button {
        time_usec: u64,
        button: u32,
        state: KeyState,
    },

    AxisPx {
        dist: Fixed,
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
    fn gtx_api(&self) -> GfxApi;
    fn version(&self) -> Result<DrmVersion, DrmError>;
    fn set_direct_scanout_enabled(&self, enabled: bool);
    fn is_render_device(&self) -> bool;
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
}

pub trait BackendDrmLease {
    fn fd(&self) -> &Rc<OwnedFd>;
}

pub trait BackendDrmLessee {
    fn created(&self, lease: Rc<dyn BackendDrmLease>);
}
