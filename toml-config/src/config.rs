mod context;
pub mod error;
mod extractor;
mod keysyms;
mod parser;
mod parsers;
mod spanned;
mod value;

use {
    crate::{
        config::{
            context::Context,
            parsers::{
                color_management::ColorManagement,
                config::{ConfigParser, ConfigParserError},
                float::Float,
            },
        },
        toml::{self},
    },
    ahash::AHashMap,
    jay_config::{
        Axis, Direction, Workspace,
        input::{SwitchEvent, acceleration::AccelProfile},
        keyboard::{Keymap, ModifiedKeySym, mods::Modifiers},
        logging::LogLevel,
        status::MessageFormat,
        theme::Color,
        video::{ColorSpace, Format, GfxApi, TearingMode, TransferFunction, Transform, VrrMode},
        xwayland::XScalingMode,
    },
    std::{
        error::Error,
        fmt::{Display, Formatter},
        rc::Rc,
        time::Duration,
    },
    thiserror::Error,
    toml::toml_parser,
};

#[derive(Debug, Copy, Clone)]
pub enum SimpleCommand {
    Close,
    DisablePointerConstraint,
    Focus(Direction),
    FocusParent,
    Move(Direction),
    None,
    Quit,
    ReloadConfigSo,
    ReloadConfigToml,
    Split(Axis),
    ToggleFloating,
    ToggleFullscreen,
    ToggleMono,
    ToggleSplit,
    Forward(bool),
    EnableWindowManagement(bool),
    SetFloatAboveFullscreen(bool),
    ToggleFloatAboveFullscreen,
    SetFloatPinned(bool),
    ToggleFloatPinned,
}

#[derive(Debug, Clone)]
#[expect(clippy::enum_variant_names)]
pub enum Action {
    ConfigureConnector {
        con: ConfigConnector,
    },
    ConfigureDirectScanout {
        enabled: bool,
    },
    ConfigureDrmDevice {
        dev: ConfigDrmDevice,
    },
    ConfigureIdle {
        idle: Option<Duration>,
        grace_period: Option<Duration>,
    },
    ConfigureInput {
        input: Box<Input>,
    },
    ConfigureOutput {
        out: Output,
    },
    Exec {
        exec: Exec,
    },
    MoveToWorkspace {
        name: String,
    },
    Multi {
        actions: Vec<Action>,
    },
    SetEnv {
        env: Vec<(String, String)>,
    },
    SetGfxApi {
        api: GfxApi,
    },
    SetKeymap {
        map: ConfigKeymap,
    },
    SetLogLevel {
        level: LogLevel,
    },
    SetRenderDevice {
        dev: Box<DrmDeviceMatch>,
    },
    SetStatus {
        status: Option<Status>,
    },
    SetTheme {
        theme: Box<Theme>,
    },
    ShowWorkspace {
        name: String,
    },
    SimpleCommand {
        cmd: SimpleCommand,
    },
    SwitchToVt {
        num: u32,
    },
    UnsetEnv {
        env: Vec<String>,
    },
    MoveToOutput {
        workspace: Option<Workspace>,
        output: OutputMatch,
    },
    SetRepeatRate {
        rate: RepeatRate,
    },
    DefineAction {
        name: String,
        action: Box<Action>,
    },
    UndefineAction {
        name: String,
    },
    NamedAction {
        name: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Theme {
    pub attention_requested_bg_color: Option<Color>,
    pub bg_color: Option<Color>,
    pub bar_bg_color: Option<Color>,
    pub bar_status_text_color: Option<Color>,
    pub border_color: Option<Color>,
    pub captured_focused_title_bg_color: Option<Color>,
    pub captured_unfocused_title_bg_color: Option<Color>,
    pub focused_inactive_title_bg_color: Option<Color>,
    pub focused_inactive_title_text_color: Option<Color>,
    pub focused_title_bg_color: Option<Color>,
    pub focused_title_text_color: Option<Color>,
    pub separator_color: Option<Color>,
    pub unfocused_title_bg_color: Option<Color>,
    pub unfocused_title_text_color: Option<Color>,
    pub highlight_color: Option<Color>,
    pub border_width: Option<i32>,
    pub title_height: Option<i32>,
    pub font: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub format: MessageFormat,
    pub exec: Exec,
    pub separator: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UiDrag {
    pub enabled: Option<bool>,
    pub threshold: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum OutputMatch {
    Any(Vec<OutputMatch>),
    All {
        name: Option<String>,
        connector: Option<String>,
        serial_number: Option<String>,
        manufacturer: Option<String>,
        model: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum DrmDeviceMatch {
    Any(Vec<DrmDeviceMatch>),
    All {
        name: Option<String>,
        syspath: Option<String>,
        vendor: Option<u32>,
        vendor_name: Option<String>,
        model: Option<u32>,
        model_name: Option<String>,
        devnode: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate: Option<f64>,
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} x {}", self.width, self.height)?;
        if let Some(rr) = self.refresh_rate {
            write!(f, " @ {}", rr)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Output {
    pub name: Option<String>,
    pub match_: OutputMatch,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub scale: Option<f64>,
    pub transform: Option<Transform>,
    pub mode: Option<Mode>,
    pub vrr: Option<Vrr>,
    pub tearing: Option<Tearing>,
    pub format: Option<Format>,
    pub color_space: Option<ColorSpace>,
    pub transfer_function: Option<TransferFunction>,
    pub brightness: Option<Option<f64>>,
}

#[derive(Debug, Clone)]
pub enum ConnectorMatch {
    Any(Vec<ConnectorMatch>),
    All { connector: Option<String> },
}

#[derive(Debug, Clone)]
pub enum InputMatch {
    Any(Vec<InputMatch>),
    All {
        tag: Option<String>,
        name: Option<String>,
        syspath: Option<String>,
        devnode: Option<String>,
        is_keyboard: Option<bool>,
        is_pointer: Option<bool>,
        is_touch: Option<bool>,
        is_tablet_tool: Option<bool>,
        is_tablet_pad: Option<bool>,
        is_gesture: Option<bool>,
        is_switch: Option<bool>,
    },
}

#[derive(Debug, Clone)]
pub struct Input {
    pub tag: Option<String>,
    pub match_: InputMatch,
    pub accel_profile: Option<AccelProfile>,
    pub accel_speed: Option<f64>,
    pub tap_enabled: Option<bool>,
    pub tap_drag_enabled: Option<bool>,
    pub tap_drag_lock_enabled: Option<bool>,
    pub left_handed: Option<bool>,
    pub natural_scrolling: Option<bool>,
    pub px_per_wheel_scroll: Option<f64>,
    pub transform_matrix: Option<[[f64; 2]; 2]>,
    pub keymap: Option<ConfigKeymap>,
    pub switch_actions: AHashMap<SwitchEvent, Action>,
    pub output: Option<Option<OutputMatch>>,
    pub calibration_matrix: Option<[[f32; 3]; 2]>,
}

#[derive(Debug, Clone)]
pub struct Exec {
    pub prog: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub privileged: bool,
}

#[derive(Debug, Clone)]
pub struct ConfigConnector {
    pub match_: ConnectorMatch,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ConfigDrmDevice {
    pub name: Option<String>,
    pub match_: DrmDeviceMatch,
    pub gfx_api: Option<GfxApi>,
    pub direct_scanout_enabled: Option<bool>,
    pub flip_margin_ms: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum ConfigKeymap {
    Named(String),
    Literal(Keymap),
    Defined { name: String, map: Keymap },
}

#[derive(Debug, Clone)]
pub struct RepeatRate {
    pub rate: i32,
    pub delay: i32,
}

#[derive(Debug, Clone)]
pub struct Vrr {
    pub mode: Option<VrrMode>,
    pub cursor_hz: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Xwayland {
    pub scaling_mode: Option<XScalingMode>,
}

#[derive(Debug, Clone)]
pub struct Tearing {
    pub mode: Option<TearingMode>,
}

#[derive(Debug, Clone, Default)]
pub struct Libei {
    pub enable_socket: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub mask: Modifiers,
    pub keysym: ModifiedKeySym,
    pub action: Action,
    pub latch: Option<Action>,
}

#[derive(Debug, Clone)]
pub struct NamedAction {
    pub name: Rc<String>,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub keymap: Option<ConfigKeymap>,
    pub repeat_rate: Option<RepeatRate>,
    pub shortcuts: Vec<Shortcut>,
    pub on_graphics_initialized: Option<Action>,
    pub on_idle: Option<Action>,
    pub status: Option<Status>,
    pub connectors: Vec<ConfigConnector>,
    pub outputs: Vec<Output>,
    pub workspace_capture: bool,
    pub env: Vec<(String, String)>,
    pub on_startup: Option<Action>,
    pub keymaps: Vec<ConfigKeymap>,
    pub log_level: Option<LogLevel>,
    pub theme: Theme,
    pub gfx_api: Option<GfxApi>,
    pub direct_scanout_enabled: Option<bool>,
    pub drm_devices: Vec<ConfigDrmDevice>,
    pub render_device: Option<DrmDeviceMatch>,
    pub inputs: Vec<Input>,
    pub idle: Option<Duration>,
    pub grace_period: Option<Duration>,
    pub explicit_sync_enabled: Option<bool>,
    pub focus_follows_mouse: bool,
    pub window_management_key: Option<ModifiedKeySym>,
    pub vrr: Option<Vrr>,
    pub tearing: Option<Tearing>,
    pub libei: Libei,
    pub ui_drag: UiDrag,
    pub xwayland: Option<Xwayland>,
    pub color_management: Option<ColorManagement>,
    pub float: Option<Float>,
    pub named_actions: Vec<NamedAction>,
    pub max_action_depth: u64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not parse the toml document")]
    Toml(#[from] toml_parser::ParserError),
    #[error("Could not interpret the toml as a config document")]
    Parser(#[from] ConfigParserError),
}

pub fn parse_config<F>(input: &[u8], handle_error: F) -> Option<Config>
where
    F: FnOnce(&dyn Error),
{
    let cx = Context {
        input,
        used: Default::default(),
    };
    macro_rules! fatal {
        ($e:expr) => {{
            let e = ConfigError::from($e.value);
            let e = cx.error2($e.span, e);
            handle_error(&e);
            return None;
        }};
    }
    let toml = match toml_parser::parse(input, &cx) {
        Ok(t) => t,
        Err(e) => fatal!(e),
    };
    let config = match toml.parse(&mut ConfigParser(&cx)) {
        Ok(c) => c,
        Err(e) => fatal!(e),
    };
    let used = cx.used.take();
    macro_rules! check_defined {
        ($name:expr, $used:ident, $defined:ident) => {
            for spanned in &used.$used {
                if !used.$defined.contains(spanned) {
                    log::warn!(
                        "{} {} used but not defined: {}",
                        $name,
                        spanned.value,
                        cx.error3(spanned.span),
                    );
                }
            }
        };
    }
    check_defined!("Keymap", keymaps, defined_keymaps);
    check_defined!("DRM device", drm_devices, defined_drm_devices);
    check_defined!("Output", outputs, defined_outputs);
    check_defined!("Input", inputs, defined_inputs);
    Some(config)
}

#[test]
fn default_config_parses() {
    let input = include_bytes!("default-config.toml");
    parse_config(input, |_| ()).unwrap();
}
