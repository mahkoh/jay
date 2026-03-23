use {
    crate::ifs::jay_tree_query::{
        TREE_TY_CONTAINER, TREE_TY_DISPLAY, TREE_TY_FLOAT, TREE_TY_LAYER_SURFACE,
        TREE_TY_LOCK_SURFACE, TREE_TY_OUTPUT, TREE_TY_PLACEHOLDER, TREE_TY_WORKSPACE,
        TREE_TY_X_WINDOW, TREE_TY_XDG_POPUP, TREE_TY_XDG_TOPLEVEL,
    },
    jay_config::video::{TearingMode, VrrMode},
    num_traits::Zero,
    serde::{Serialize, Serializer},
    std::{
        io::{Write, stdout},
        sync::atomic::{AtomicBool, Ordering::Relaxed},
    },
    uapi::c,
};

pub static VERBOSE_JSON: AtomicBool = AtomicBool::new(false);

fn quiet() -> bool {
    !VERBOSE_JSON.load(Relaxed)
}

fn is_none<T>(t: &Option<T>) -> bool {
    quiet() && t.is_none()
}

fn is_empty<T>(t: &[T]) -> bool {
    quiet() && t.is_empty()
}

fn is_false(v: &bool) -> bool {
    quiet() && !*v
}

fn is_zero(v: &impl Zero) -> bool {
    quiet() && v.is_zero()
}

pub fn jsonl<T>(value: &T)
where
    T: ?Sized + Serialize,
{
    let mut writer = stdout().lock();
    serde_json::to_writer(&mut writer, value).unwrap();
    writer.write_all(b"\n").unwrap();
}

#[derive(Serialize)]
pub struct JsonClient<'a> {
    pub client_id: u64,
    #[serde(skip_serializing_if = "is_false")]
    pub sandboxed: bool,
    #[serde(skip_serializing_if = "is_none")]
    pub sandbox_engine: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub sandbox_app_id: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub sandbox_instance_id: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub uid: Option<c::uid_t>,
    #[serde(skip_serializing_if = "is_none")]
    pub pid: Option<c::pid_t>,
    #[serde(skip_serializing_if = "is_false")]
    pub is_xwayland: bool,
    #[serde(skip_serializing_if = "is_none")]
    pub comm: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub exe: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub tag: Option<&'a str>,
}

#[derive(Serialize)]
pub struct JsonColorManagementStatus {
    pub enabled: bool,
    pub available: bool,
}

#[derive(Serialize)]
pub struct JsonIdle<'a> {
    pub idle_sec: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub grace_sec: u64,
    #[serde(skip_serializing_if = "is_empty")]
    pub inhibitors: Vec<JsonIdleInhibitor<'a>>,
}

#[derive(Serialize)]
pub struct JsonIdleInhibitor<'a> {
    pub client_id: u64,
    pub surface: u32,
    pub pid: u64,
    pub comm: &'a str,
}

#[derive(Serialize)]
pub struct JsonRandrData<'a> {
    #[serde(skip_serializing_if = "is_empty")]
    pub drm_devices: Vec<JsonDrmDevice<'a>>,
    #[serde(skip_serializing_if = "is_empty")]
    pub unbound_connectors: Vec<JsonConnector<'a>>,
}

#[derive(Serialize)]
pub struct JsonDrmDevice<'a> {
    pub devnode: &'a str,
    pub syspath: &'a str,
    pub vendor: u32,
    pub vendor_name: &'a str,
    pub model: u32,
    pub model_name: &'a str,
    pub gfx_api: &'a str,
    #[serde(skip_serializing_if = "is_false")]
    pub render_device: bool,
    #[serde(skip_serializing_if = "is_empty")]
    pub connectors: Vec<JsonConnector<'a>>,
}

#[derive(Serialize)]
pub struct JsonConnector<'a> {
    pub name: &'a str,
    pub enabled: bool,
    #[serde(skip_serializing_if = "is_none")]
    pub output: Option<JsonOutput<'a>>,
}

pub struct JsonVrrMode(pub VrrMode);

impl Serialize for JsonVrrMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self.0 {
            VrrMode::NEVER => "never",
            VrrMode::ALWAYS => "always",
            VrrMode::VARIANT_1 => "variant1",
            VrrMode::VARIANT_2 => "variant2",
            VrrMode::VARIANT_3 => "variant3",
            n => return serializer.serialize_u32(n.0),
        };
        serializer.serialize_str(s)
    }
}

pub struct JsonTearingMode(pub TearingMode);

impl Serialize for JsonTearingMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self.0 {
            TearingMode::NEVER => "never",
            TearingMode::ALWAYS => "always",
            TearingMode::VARIANT_1 => "variant1",
            TearingMode::VARIANT_2 => "variant2",
            TearingMode::VARIANT_3 => "variant3",
            n => return serializer.serialize_u32(n.0),
        };
        serializer.serialize_str(s)
    }
}

#[derive(Serialize)]
pub struct JsonOutput<'a> {
    pub product: &'a str,
    pub manufacturer: &'a str,
    pub serial_number: &'a str,
    #[serde(skip_serializing_if = "is_zero")]
    pub width_mm: i32,
    #[serde(skip_serializing_if = "is_zero")]
    pub height_mm: i32,
    #[serde(skip_serializing_if = "is_false")]
    pub non_desktop: bool,
    pub scale: f64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub transform: &'static str,
    #[serde(skip_serializing_if = "is_none")]
    pub mode: Option<JsonMode>,
    #[serde(skip_serializing_if = "is_none")]
    pub format: Option<&'a str>,
    #[serde(skip_serializing_if = "is_false")]
    pub vrr_capable: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub vrr_enabled: bool,
    pub vrr_mode: JsonVrrMode,
    #[serde(skip_serializing_if = "is_none")]
    pub vrr_cursor_hz: Option<f64>,
    pub tearing_mode: JsonTearingMode,
    #[serde(skip_serializing_if = "is_none")]
    pub flip_margin_ns: Option<u64>,
    #[serde(skip_serializing_if = "is_empty")]
    pub supported_color_spaces: Vec<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub current_color_space: Option<&'a str>,
    #[serde(skip_serializing_if = "is_empty")]
    pub supported_eotfs: Vec<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub current_eotf: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub min_brightness: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub max_brightness: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub brightness: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub blend_space: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub native_gamut: Option<JsonPrimaries>,
    #[serde(skip_serializing_if = "is_false")]
    pub use_native_gamut: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub arbitrary_modes: bool,
    #[serde(skip_serializing_if = "is_empty")]
    pub modes: Vec<JsonMode>,
    #[serde(skip_serializing_if = "is_empty")]
    pub formats: Vec<&'a str>,
}

#[derive(Serialize)]
pub struct JsonMode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate_millihz: u32,
    #[serde(skip_serializing_if = "is_false")]
    pub current: bool,
}

#[derive(Serialize)]
pub struct JsonPrimaries {
    pub r_x: f64,
    pub r_y: f64,
    pub g_x: f64,
    pub g_y: f64,
    pub b_x: f64,
    pub b_y: f64,
    pub w_x: f64,
    pub w_y: f64,
}

#[derive(Serialize)]
pub struct JsonInputData<'a> {
    #[serde(skip_serializing_if = "is_empty")]
    pub seats: Vec<JsonSeat<'a>>,
    #[serde(skip_serializing_if = "is_empty")]
    pub detached_devices: Vec<JsonInputDevice<'a>>,
}

#[derive(Serialize)]
pub struct JsonSeat<'a> {
    pub name: &'a str,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    #[serde(skip_serializing_if = "is_false")]
    pub hardware_cursor: bool,
    #[serde(skip_serializing_if = "is_empty")]
    pub devices: Vec<JsonInputDevice<'a>>,
}

#[derive(Serialize)]
pub struct JsonInputDevice<'a> {
    pub input_device_id: u32,
    pub name: &'a str,
    #[serde(skip_serializing_if = "is_none")]
    pub seat: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub syspath: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub devnode: Option<&'a str>,
    #[serde(skip_serializing_if = "is_empty")]
    pub capabilities: Vec<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub accel_profile: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub accel_speed: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub tap_enabled: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub tap_drag_enabled: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub tap_drag_lock_enabled: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub left_handed: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub natural_scrolling: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub px_per_wheel_scroll: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub transform_matrix: Option<[[f64; 2]; 2]>,
    #[serde(skip_serializing_if = "is_none")]
    pub output: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub calibration_matrix: Option<[[f32; 3]; 2]>,
    #[serde(skip_serializing_if = "is_none")]
    pub click_method: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub middle_button_emulation: Option<bool>,
}

pub struct JsonTreeNodeType(pub u32);

impl Serialize for JsonTreeNodeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self.0 {
            TREE_TY_DISPLAY => "display",
            TREE_TY_OUTPUT => "output",
            TREE_TY_WORKSPACE => "workspace",
            TREE_TY_FLOAT => "float",
            TREE_TY_CONTAINER => "container",
            TREE_TY_PLACEHOLDER => "placeholder",
            TREE_TY_XDG_TOPLEVEL => "xdg-toplevel",
            TREE_TY_X_WINDOW => "x-window",
            TREE_TY_XDG_POPUP => "xdg-popup",
            TREE_TY_LAYER_SURFACE => "layer-surface",
            TREE_TY_LOCK_SURFACE => "lock-surface",
            n => return serializer.serialize_u32(n),
        };
        serializer.serialize_str(s)
    }
}

#[derive(Serialize)]
pub struct JsonTreeNode<'a> {
    #[serde(rename = "type")]
    pub ty: JsonTreeNodeType,
    #[serde(skip_serializing_if = "is_none")]
    pub output: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub workspace: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub toplevel_id: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub placeholder_for: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub position: Option<JsonRect>,
    #[serde(skip_serializing_if = "is_none")]
    pub client: Option<JsonClient<'a>>,
    #[serde(skip_serializing_if = "is_none")]
    pub title: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub app_id: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub tag: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub content_type: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub x_class: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub x_instance: Option<&'a str>,
    #[serde(skip_serializing_if = "is_none")]
    pub x_role: Option<&'a str>,
    #[serde(skip_serializing_if = "is_false")]
    pub floating: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub visible: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub urgent: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub fullscreen: bool,
    #[serde(skip_serializing_if = "is_empty")]
    pub children: Vec<JsonTreeNode<'a>>,
}

#[derive(Serialize)]
pub struct JsonRect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize)]
pub struct JsonXwaylandStatus<'a> {
    pub scaling_mode: &'a str,
    #[serde(skip_serializing_if = "is_none")]
    pub implied_scale: Option<f64>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum JsonSeatEvent<'a> {
    Key {
        seat: &'a str,
        time_usec: u64,
        key: u32,
        state: u32,
    },
    Modifiers {
        seat: &'a str,
        modifiers: u32,
        group: u32,
    },
    PointerAbs {
        seat: &'a str,
        time_usec: u64,
        x: f64,
        y: f64,
    },
    PointerRel {
        seat: &'a str,
        time_usec: u64,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        dx_unaccelerated: f64,
        dy_unaccelerated: f64,
    },
    Button {
        seat: &'a str,
        time_usec: u64,
        button: u32,
        state: u32,
    },
    Axis {
        seat: &'a str,
        time_usec: u64,
        #[serde(skip_serializing_if = "is_none")]
        source: Option<&'a str>,
        #[serde(skip_serializing_if = "is_none")]
        horizontal: Option<JsonAxisData>,
        #[serde(skip_serializing_if = "is_none")]
        vertical: Option<JsonAxisData>,
    },
    SwipeBegin {
        seat: &'a str,
        time_usec: u64,
        fingers: u32,
    },
    SwipeUpdate {
        seat: &'a str,
        time_usec: u64,
        dx: f64,
        dy: f64,
        dx_unaccelerated: f64,
        dy_unaccelerated: f64,
    },
    SwipeEnd {
        seat: &'a str,
        time_usec: u64,
        cancelled: bool,
    },
    PinchBegin {
        seat: &'a str,
        time_usec: u64,
        fingers: u32,
    },
    PinchUpdate {
        seat: &'a str,
        time_usec: u64,
        dx: f64,
        dy: f64,
        dx_unaccelerated: f64,
        dy_unaccelerated: f64,
        scale: f64,
        rotation: f64,
    },
    PinchEnd {
        seat: &'a str,
        time_usec: u64,
        cancelled: bool,
    },
    HoldBegin {
        seat: &'a str,
        time_usec: u64,
        fingers: u32,
    },
    HoldEnd {
        seat: &'a str,
        time_usec: u64,
        cancelled: bool,
    },
    Switch {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        event: &'a str,
    },
    TabletTool {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        tool: u32,
        #[serde(skip_serializing_if = "is_false")]
        proximity_in: bool,
        #[serde(skip_serializing_if = "is_false")]
        proximity_out: bool,
        #[serde(skip_serializing_if = "is_false")]
        down: bool,
        #[serde(skip_serializing_if = "is_false")]
        up: bool,
        #[serde(skip_serializing_if = "is_none")]
        x: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        y: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        pressure: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        distance: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        tilt_x: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        tilt_y: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        rotation: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        slider: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        wheel_degrees: Option<f64>,
        #[serde(skip_serializing_if = "is_none")]
        wheel_clicks: Option<i32>,
        #[serde(skip_serializing_if = "is_none")]
        button: Option<u32>,
        #[serde(skip_serializing_if = "is_none")]
        button_state: Option<&'a str>,
    },
    TabletPadModeSwitch {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        mode: u32,
    },
    TabletPadButton {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        button: u32,
        state: &'a str,
    },
    TabletPadStrip {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        strip: u32,
        source: &'a str,
        #[serde(skip_serializing_if = "is_none")]
        position: Option<f64>,
        #[serde(skip_serializing_if = "is_false")]
        stop: bool,
    },
    TabletPadRing {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        ring: u32,
        source: &'a str,
        #[serde(skip_serializing_if = "is_none")]
        degrees: Option<f64>,
        #[serde(skip_serializing_if = "is_false")]
        stop: bool,
    },
    TabletPadDial {
        seat: &'a str,
        time_usec: u64,
        input_device: u32,
        dial: u32,
        #[serde(skip_serializing_if = "is_none")]
        delta120: Option<i32>,
    },
    TouchDown {
        seat: &'a str,
        time_usec: u64,
        id: i32,
        x: f64,
        y: f64,
    },
    TouchUp {
        seat: &'a str,
        time_usec: u64,
        id: i32,
    },
    TouchMotion {
        seat: &'a str,
        time_usec: u64,
        id: i32,
        x: f64,
        y: f64,
    },
    TouchCancel {
        seat: &'a str,
        time_usec: u64,
        id: i32,
    },
}

#[derive(Serialize)]
pub struct JsonAxisData {
    #[serde(skip_serializing_if = "is_none")]
    pub px: Option<f64>,
    #[serde(skip_serializing_if = "is_none")]
    pub v120: Option<i32>,
    #[serde(skip_serializing_if = "is_false")]
    pub stop: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub natural_scrolling: bool,
}
