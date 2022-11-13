use {
    crate::{
        async_engine::SpawnedFuture,
        fixed::Fixed,
        ifs::wl_seat::wl_pointer::{CONTINUOUS, FINGER, HORIZONTAL_SCROLL, VERTICAL_SCROLL, WHEEL},
        render::Framebuffer,
        video::drm::{ConnectorType, DrmError, DrmVersion},
    },
    std::{
        any::Any,
        error::Error,
        fmt::{Debug, Display, Formatter},
        rc::Rc,
    },
    uapi::c,
};

linear_ids!(ConnectorIds, ConnectorId);
linear_ids!(InputDeviceIds, InputDeviceId);
linear_ids!(DrmDeviceIds, DrmDeviceId);

pub trait Backend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>>;
    fn into_any(self: Rc<Self>) -> Rc<dyn Any>;

    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn set_idle(&self, idle: bool) {
        let _ = idle;
    }

    fn supports_idle(&self) -> bool {
        false
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
    fn set_enabled(&self, enabled: bool);
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Connected(MonitorInfo),
    HardwareCursor(Option<Rc<dyn HardwareCursor>>),
    Disconnected,
    Removed,
    ModeChanged(Mode),
}

pub trait HardwareCursor: Debug {
    fn set_enabled(&self, enabled: bool);
    fn get_buffer(&self) -> Rc<Framebuffer>;
    fn set_position(&self, x: i32, y: i32);
    fn swap_buffer(&self);
    fn commit(&self);
    fn max_size(&self) -> (i32, i32);
}

pub type TransformMatrix = [[f64; 2]; 2];

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
    fn set_transform_matrix(&self, matrix: TransformMatrix);
    fn name(&self) -> Rc<String>;
    fn set_tap_enabled(&self, enabled: bool);
    fn set_drag_enabled(&self, enabled: bool);
    fn set_drag_lock_enabled(&self, enabled: bool);
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
    },
    AxisFrame {
        time_usec: u64,
    },
}

pub enum DrmEvent {
    #[allow(dead_code)]
    Removed,
}

pub trait BackendDrmDevice {
    fn id(&self) -> DrmDeviceId;
    fn event(&self) -> Option<DrmEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn dev_t(&self) -> c::dev_t;
    fn make_render_device(self: Rc<Self>);
    fn version(&self) -> Result<DrmVersion, DrmError>;
}
