use {
    crate::{
        fixed::Fixed,
        ifs::wl_seat::wl_pointer::{CONTINUOUS, FINGER, HORIZONTAL_SCROLL, VERTICAL_SCROLL, WHEEL},
        video::drm::ConnectorType,
    },
    std::{
        fmt::{Debug, Display, Formatter},
        rc::Rc,
    },
};

linear_ids!(ConnectorIds, ConnectorId);
linear_ids!(InputDeviceIds, InputDeviceId);

pub trait Backend {
    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn set_idle(&self, idle: bool) {
        let _ = idle;
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate_millihz: u32,
}

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub modes: Vec<Mode>,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
    pub initial_mode: Mode,
    pub width_mm: i32,
    pub height_mm: i32,
}

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
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected(MonitorInfo),
    Disconnected,
    Removed,
    ModeChanged(Mode),
}

pub trait InputDevice {
    fn id(&self) -> InputDeviceId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<InputEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn grab(&self, grab: bool);
    fn has_capability(&self, cap: InputDeviceCapability) -> bool;
    fn set_left_handed(&self, left_handed: bool);
    fn set_accel_profile(&self, profile: InputDeviceAccelProfile);
    fn set_accel_speed(&self, speed: f64);
    fn set_transform_matrix(&self, matrix: [[f64; 2]; 2]);
    fn name(&self) -> Rc<String>;
}

#[derive(Debug, Copy, Clone)]
pub enum InputDeviceCapability {
    Keyboard,
    Pointer,
    Touch,
    TabletTool,
    TabletPad,
    Gesture,
    Switch,
}

#[derive(Debug, Copy, Clone)]
pub enum InputDeviceAccelProfile {
    Flat,
    Adaptive,
}

pub enum BackendEvent {
    NewConnector(Rc<dyn Connector>),
    NewInputDevice(Rc<dyn InputDevice>),
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

#[derive(Debug)]
pub enum InputEvent {
    Key(u32, KeyState),
    ConnectorPosition(ConnectorId, Fixed, Fixed),
    #[allow(dead_code)]
    Motion(Fixed, Fixed),
    Button(u32, KeyState),

    Axis(Fixed, ScrollAxis),
    AxisSource(AxisSource),
    AxisStop(ScrollAxis),
    AxisDiscrete(i32, ScrollAxis),
    Frame,
}
