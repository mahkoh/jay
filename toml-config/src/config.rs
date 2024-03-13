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
            parsers::config::{ConfigParser, ConfigParserError},
        },
        toml::{self},
    },
    jay_config::{
        input::acceleration::AccelProfile,
        keyboard::{Keymap, ModifiedKeySym},
        logging::LogLevel,
        status::MessageFormat,
        theme::Color,
        video::{GfxApi, Transform},
        Axis, Direction,
    },
    std::{
        error::Error,
        fmt::{Display, Formatter},
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
}

#[derive(Debug, Clone)]
pub enum Action {
    ConfigureConnector { con: ConfigConnector },
    ConfigureDirectScanout { enabled: bool },
    ConfigureDrmDevice { dev: ConfigDrmDevice },
    ConfigureIdle { idle: Duration },
    ConfigureInput { input: Input },
    ConfigureOutput { out: Output },
    Exec { exec: Exec },
    Multi { actions: Vec<Action> },
    SetEnv { env: Vec<(String, String)> },
    SetGfxApi { api: GfxApi },
    SetKeymap { map: ConfigKeymap },
    SetLogLevel { level: LogLevel },
    SetRenderDevice { dev: DrmDeviceMatch },
    SetStatus { status: Option<Status> },
    SetTheme { theme: Box<Theme> },
    ShowWorkspace { name: String },
    SimpleCommand { cmd: SimpleCommand },
    SwitchToVt { num: u32 },
    UnsetEnv { env: Vec<String> },
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
}

#[derive(Debug, Clone)]
pub struct Exec {
    pub prog: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
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
}

#[derive(Debug, Clone)]
pub enum ConfigKeymap {
    Named(String),
    Literal(Keymap),
    Defined { name: String, map: Keymap },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub keymap: Option<ConfigKeymap>,
    pub shortcuts: Vec<(ModifiedKeySym, Action)>,
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
