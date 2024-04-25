use {
    crate::{
        async_engine::SpawnedFuture,
        drm_feedback::DrmFeedback,
        fixed::Fixed,
        gfx_api::{GfxFramebuffer, SyncFile},
        ifs::wl_seat::wl_pointer::{CONTINUOUS, FINGER, HORIZONTAL_SCROLL, VERTICAL_SCROLL, WHEEL},
        libinput::consts::DeviceCapability,
        video::drm::{ConnectorType, DrmError, DrmVersion},
    },
    jay_config::video::GfxApi,
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
    pub non_desktop: bool,
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
    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer>;
    fn set_position(&self, x: i32, y: i32);
    fn swap_buffer(&self);
    fn set_sync_file(&self, sync_file: Option<SyncFile>);
    fn commit(&self);
    fn size(&self) -> (i32, i32);
}

pub type TransformMatrix = [[f64; 2]; 2];

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
}

pub enum DrmEvent {
    #[allow(dead_code)]
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
}
