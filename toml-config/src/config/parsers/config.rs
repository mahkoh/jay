use {
    crate::{
        config::{
            context::Context,
            extractor::{arr, bol, opt, recover, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::ActionParser,
                connector::ConnectorsParser,
                drm_device::DrmDevicesParser,
                drm_device_match::DrmDeviceMatchParser,
                env::EnvParser,
                gfx_api::GfxApiParser,
                idle::IdleParser,
                input::InputsParser,
                keymap::KeymapParser,
                log_level::LogLevelParser,
                output::OutputsParser,
                shortcuts::{ShortcutsParser, ShortcutsParserError},
                status::StatusParser,
                theme::ThemeParser,
            },
            spanned::SpannedErrorExt,
            Action, Config, Theme,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ConfigParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
    #[error("Could not parse the shortcuts")]
    ParseShortcuts(#[source] ShortcutsParserError),
}

pub struct ConfigParser<'a>(pub &'a Context<'a>);

impl ConfigParser<'_> {
    fn parse_action(&self, name: &str, action: Option<Spanned<&Value>>) -> Option<Action> {
        match action {
            None => None,
            Some(value) => match value.parse(&mut ActionParser(self.0)) {
                Ok(v) => Some(v),
                Err(e) => {
                    log::warn!("Could not parse the {name} action: {}", self.0.error(e));
                    None
                }
            },
        }
    }
}

impl Parser for ConfigParser<'_> {
    type Value = Config;
    type Error = ConfigParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            (
                keymap_val,
                shortcuts_val,
                on_graphics_init_val,
                status_val,
                outputs_val,
                connectors_val,
                workspace_capture,
                env_val,
                on_startup_val,
                keymaps_val,
            ),
            (
                log_level_val,
                theme_val,
                gfx_api_val,
                drm_devices_val,
                direct_scanout,
                render_device_val,
                inputs_val,
                on_idle_val,
                _,
                idle_val,
            ),
        ) = ext.extract((
            (
                opt(val("keymap")),
                opt(val("shortcuts")),
                opt(val("on-graphics-initialized")),
                opt(val("status")),
                opt(val("outputs")),
                opt(val("connectors")),
                recover(opt(bol("workspace-capture"))),
                opt(val("env")),
                opt(val("on-startup")),
                recover(opt(arr("keymaps"))),
            ),
            (
                opt(val("log-level")),
                opt(val("theme")),
                opt(val("gfx-api")),
                opt(val("drm-devices")),
                recover(opt(bol("direct-scanout"))),
                opt(val("render-device")),
                opt(val("inputs")),
                opt(val("on-idle")),
                opt(val("$schema")),
                opt(val("idle")),
            ),
        ))?;
        let mut keymap = None;
        if let Some(value) = keymap_val {
            match value.parse(&mut KeymapParser {
                cx: self.0,
                definition: false,
            }) {
                Ok(m) => keymap = Some(m),
                Err(e) => {
                    log::warn!("Could not parse the keymap: {}", self.0.error(e));
                }
            }
        }
        let mut shortcuts = vec![];
        if let Some(value) = shortcuts_val {
            shortcuts = value
                .parse(&mut ShortcutsParser(self.0))
                .map_spanned_err(ConfigParserError::ParseShortcuts)?;
        }
        if shortcuts.is_empty() {
            log::warn!("Config defines no shortcuts");
        }
        let on_graphics_initialized =
            self.parse_action("on-graphics-initialized", on_graphics_init_val);
        let on_idle = self.parse_action("on-idle", on_idle_val);
        let on_startup = self.parse_action("on-startup", on_startup_val);
        let mut status = None;
        if let Some(value) = status_val {
            match value.parse(&mut StatusParser(self.0)) {
                Ok(v) => status = Some(v),
                Err(e) => log::warn!("Could not parse the status config: {}", self.0.error(e)),
            }
        }
        let mut outputs = vec![];
        if let Some(value) = outputs_val {
            match value.parse(&mut OutputsParser(self.0)) {
                Ok(v) => outputs = v,
                Err(e) => log::warn!("Could not parse the outputs: {}", self.0.error(e)),
            }
        }
        let mut connectors = vec![];
        if let Some(value) = connectors_val {
            match value.parse(&mut ConnectorsParser(self.0)) {
                Ok(v) => connectors = v,
                Err(e) => log::warn!("Could not parse the connectors: {}", self.0.error(e)),
            }
        }
        let mut env = vec![];
        if let Some(value) = env_val {
            match value.parse(&mut EnvParser) {
                Ok(v) => env = v,
                Err(e) => log::warn!(
                    "Could not parse the environment variables: {}",
                    self.0.error(e)
                ),
            }
        }
        let mut keymaps = vec![];
        if let Some(value) = keymaps_val {
            for value in value.value {
                match value.parse(&mut KeymapParser {
                    cx: self.0,
                    definition: true,
                }) {
                    Ok(m) => keymaps.push(m),
                    Err(e) => {
                        log::warn!("Could not parse a keymap: {}", self.0.error(e));
                    }
                }
            }
        }
        let mut log_level = None;
        if let Some(value) = log_level_val {
            match value.parse(&mut LogLevelParser) {
                Ok(v) => log_level = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the log level: {}", self.0.error(e));
                }
            }
        }
        let mut theme = Theme::default();
        if let Some(value) = theme_val {
            match value.parse(&mut ThemeParser(self.0)) {
                Ok(v) => theme = v,
                Err(e) => {
                    log::warn!("Could not parse the theme: {}", self.0.error(e));
                }
            }
        }
        let mut gfx_api = None;
        if let Some(value) = gfx_api_val {
            match value.parse(&mut GfxApiParser) {
                Ok(v) => gfx_api = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the graphics API: {}", self.0.error(e));
                }
            }
        }
        let mut drm_devices = vec![];
        if let Some(value) = drm_devices_val {
            match value.parse(&mut DrmDevicesParser(self.0)) {
                Ok(v) => drm_devices = v,
                Err(e) => {
                    log::warn!("Could not parse the drm devices: {}", self.0.error(e));
                }
            }
        }
        let mut render_device = None;
        if let Some(value) = render_device_val {
            match value.parse(&mut DrmDeviceMatchParser(self.0)) {
                Ok(v) => render_device = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the render device: {}", self.0.error(e));
                }
            }
        }
        let mut inputs = vec![];
        if let Some(value) = inputs_val {
            match value.parse(&mut InputsParser(self.0)) {
                Ok(v) => inputs = v,
                Err(e) => {
                    log::warn!("Could not parse the inputs: {}", self.0.error(e));
                }
            }
        }
        let mut idle = None;
        if let Some(value) = idle_val {
            match value.parse(&mut IdleParser(self.0)) {
                Ok(v) => idle = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the idle timeout: {}", self.0.error(e));
                }
            }
        }
        Ok(Config {
            keymap,
            shortcuts,
            on_graphics_initialized,
            on_idle,
            status,
            outputs,
            connectors,
            workspace_capture: workspace_capture.despan().unwrap_or(true),
            env,
            on_startup,
            keymaps,
            log_level,
            theme,
            gfx_api,
            drm_devices,
            direct_scanout_enabled: direct_scanout.despan(),
            render_device,
            inputs,
            idle,
        })
    }
}
