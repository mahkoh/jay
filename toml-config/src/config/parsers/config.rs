use {
    crate::{
        config::{
            Action, Config, Libei, Theme, UiDrag,
            context::Context,
            extractor::{Extractor, ExtractorError, arr, bol, int, opt, recover, str, val},
            keysyms::KEYSYMS,
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::ActionParser,
                actions::ActionsParser,
                client_rule::ClientRulesParser,
                color_management::ColorManagementParser,
                connector::ConnectorsParser,
                drm_device::DrmDevicesParser,
                drm_device_match::DrmDeviceMatchParser,
                env::EnvParser,
                fallback_output_mode::FallbackOutputModeParser,
                float::FloatParser,
                focus_history::FocusHistoryParser,
                gfx_api::GfxApiParser,
                idle::IdleParser,
                input::InputsParser,
                input_mode::InputModesParser,
                keymap::KeymapParser,
                libei::LibeiParser,
                log_level::LogLevelParser,
                output::OutputsParser,
                repeat_rate::RepeatRateParser,
                shortcuts::{
                    ComplexShortcutsParser, ShortcutsParser, ShortcutsParserError,
                    parse_modified_keysym_str,
                },
                simple_im::SimpleImParser,
                status::StatusParser,
                tearing::TearingParser,
                theme::ThemeParser,
                ui_drag::UiDragParser,
                vrr::VrrParser,
                window_rule::WindowRulesParser,
                workspace_display_order::WorkspaceDisplayOrderParser,
                xwayland::XwaylandParser,
            },
            spanned::SpannedErrorExt,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    ahash::AHashMap,
    indexmap::IndexMap,
    std::collections::HashSet,
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
            (
                explicit_sync,
                repeat_rate_val,
                complex_shortcuts_val,
                focus_follows_mouse,
                window_management_key_val,
                vrr_val,
                tearing_val,
                libei_val,
                ui_drag_val,
                xwayland_val,
            ),
            (
                color_management_val,
                float_val,
                actions_val,
                max_action_depth_val,
                client_rules_val,
                window_rules_val,
                pointer_revert_key_str,
                use_hardware_cursor,
                show_bar,
                focus_history_val,
            ),
            (
                middle_click_paste,
                input_modes_val,
                workspace_display_order_val,
                auto_reload,
                simple_im_val,
                show_titles,
                fallback_output_mode_val,
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
            (
                recover(opt(bol("explicit-sync"))),
                opt(val("repeat-rate")),
                opt(val("complex-shortcuts")),
                recover(opt(bol("focus-follows-mouse"))),
                recover(opt(str("window-management-key"))),
                opt(val("vrr")),
                opt(val("tearing")),
                opt(val("libei")),
                opt(val("ui-drag")),
                opt(val("xwayland")),
            ),
            (
                opt(val("color-management")),
                opt(val("float")),
                opt(val("actions")),
                recover(opt(int("max-action-depth"))),
                opt(val("clients")),
                opt(val("windows")),
                recover(opt(str("pointer-revert-key"))),
                recover(opt(bol("use-hardware-cursor"))),
                recover(opt(bol("show-bar"))),
                opt(val("focus-history")),
            ),
            (
                recover(opt(bol("middle-click-paste"))),
                opt(val("modes")),
                opt(val("workspace-display-order")),
                recover(opt(bol("auto-reload"))),
                opt(val("simple-im")),
                recover(opt(bol("show-titles"))),
                opt(val("fallback-output-mode")),
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
        let mut used_keys = HashSet::new();
        let mut shortcuts = vec![];
        if let Some(value) = shortcuts_val {
            value
                .parse(&mut ShortcutsParser {
                    cx: self.0,
                    used_keys: &mut used_keys,
                    shortcuts: &mut shortcuts,
                })
                .map_spanned_err(ConfigParserError::ParseShortcuts)?;
        }
        if let Some(value) = complex_shortcuts_val {
            value
                .parse(&mut ComplexShortcutsParser {
                    cx: self.0,
                    used_keys: &mut used_keys,
                    shortcuts: &mut shortcuts,
                })
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
        let mut grace_period = None;
        if let Some(value) = idle_val {
            match value.parse(&mut IdleParser(self.0)) {
                Ok(v) => {
                    idle = v.timeout;
                    grace_period = v.grace_period;
                }
                Err(e) => {
                    log::warn!("Could not parse the idle timeout: {}", self.0.error(e));
                }
            }
        }
        let mut repeat_rate = None;
        if let Some(value) = repeat_rate_val {
            match value.parse(&mut RepeatRateParser(self.0)) {
                Ok(v) => repeat_rate = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the repeat rate: {}", self.0.error(e));
                }
            }
        }
        let mut window_management_key = None;
        if let Some(value) = window_management_key_val
            && let Some(key) = parse_modified_keysym_str(self.0, value.span, value.value)
        {
            window_management_key = Some(key);
        }
        let mut vrr = None;
        if let Some(value) = vrr_val {
            match value.parse(&mut VrrParser(self.0)) {
                Ok(v) => vrr = Some(v),
                Err(e) => {
                    log::warn!("Could not parse VRR setting: {}", self.0.error(e));
                }
            }
        }
        let mut tearing = None;
        if let Some(value) = tearing_val {
            match value.parse(&mut TearingParser(self.0)) {
                Ok(v) => tearing = Some(v),
                Err(e) => {
                    log::warn!("Could not parse tearing setting: {}", self.0.error(e));
                }
            }
        }
        let mut libei = Libei::default();
        if let Some(value) = libei_val {
            match value.parse(&mut LibeiParser(self.0)) {
                Ok(v) => libei = v,
                Err(e) => {
                    log::warn!("Could not parse libei setting: {}", self.0.error(e));
                }
            }
        }
        let mut ui_drag = UiDrag::default();
        if let Some(value) = ui_drag_val {
            match value.parse(&mut UiDragParser(self.0)) {
                Ok(v) => ui_drag = v,
                Err(e) => {
                    log::warn!("Could not parse ui-drag setting: {}", self.0.error(e));
                }
            }
        }
        let mut xwayland = None;
        if let Some(value) = xwayland_val {
            match value.parse(&mut XwaylandParser(self.0)) {
                Ok(v) => xwayland = Some(v),
                Err(e) => {
                    log::warn!("Could not parse Xwayland setting: {}", self.0.error(e));
                }
            }
        }
        let mut color_management = None;
        if let Some(value) = color_management_val {
            match value.parse(&mut ColorManagementParser(self.0)) {
                Ok(v) => color_management = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the color-management settings: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        let mut float = None;
        if let Some(value) = float_val {
            match value.parse(&mut FloatParser(self.0)) {
                Ok(v) => float = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the float settings: {}", self.0.error(e));
                }
            }
        }
        let mut named_actions = vec![];
        if let Some(value) = actions_val {
            let mut parser = ActionsParser {
                cx: self.0,
                used_names: Default::default(),
                actions: &mut named_actions,
            };
            if let Err(e) = value.parse(&mut parser) {
                log::warn!("Could not parse named actions: {}", self.0.error(e));
            }
        }
        let mut max_action_depth = 16;
        if let Some(mut value) = max_action_depth_val {
            if value.value < 0 {
                log::warn!(
                    "Max action depth should not be negative: {}",
                    self.0.error3(value.span)
                );
                value.value = 0;
            }
            max_action_depth = value.value as _;
        }
        let mut client_rules = vec![];
        if let Some(value) = client_rules_val {
            match value.parse(&mut ClientRulesParser(self.0)) {
                Ok(v) => client_rules = v,
                Err(e) => log::warn!("Could not parse the client rules: {}", self.0.error(e)),
            }
        }
        let mut window_rules = vec![];
        if let Some(value) = window_rules_val {
            match value.parse(&mut WindowRulesParser(self.0)) {
                Ok(v) => window_rules = v,
                Err(e) => log::warn!("Could not parse the window rules: {}", self.0.error(e)),
            }
        }
        let mut pointer_revert_key = None;
        if let Some(value) = pointer_revert_key_str {
            match KEYSYMS.get(value.value) {
                Some(s) => pointer_revert_key = Some(*s),
                None => log::warn!("Unknown keysym: {}", self.0.error3(value.span)),
            }
        }
        let mut focus_history = None;
        if let Some(value) = focus_history_val {
            match value.parse(&mut FocusHistoryParser(self.0)) {
                Ok(v) => focus_history = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the focus-history settings: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        let mut input_modes = AHashMap::new();
        if let Some(value) = input_modes_val {
            match value.parse(&mut InputModesParser(self.0)) {
                Ok(v) => input_modes = v,
                Err(e) => {
                    log::warn!("Could not parse the input modes: {}", self.0.error(e),);
                }
            }
        }
        let mut workspace_display_order = None;
        if let Some(value) = workspace_display_order_val {
            match value.parse(&mut WorkspaceDisplayOrderParser) {
                Ok(v) => workspace_display_order = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the workspace display order: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        let mut simple_im = None;
        if let Some(value) = simple_im_val {
            match value.parse(&mut SimpleImParser(self.0)) {
                Ok(v) => simple_im = Some(v),
                Err(e) => {
                    log::warn!("Could not parse simple IM setting: {}", self.0.error(e));
                }
            }
        }
        let mut fallback_output_mode = None;
        if let Some(value) = fallback_output_mode_val {
            match value.parse(&mut FallbackOutputModeParser) {
                Ok(v) => fallback_output_mode = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the fallback output mode: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        Ok(Config {
            keymap,
            repeat_rate,
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
            auto_reload: auto_reload.despan(),
            log_level,
            theme,
            gfx_api,
            drm_devices,
            direct_scanout_enabled: direct_scanout.despan(),
            explicit_sync_enabled: explicit_sync.despan(),
            render_device,
            inputs,
            idle,
            grace_period,
            focus_follows_mouse: focus_follows_mouse.despan().unwrap_or(true),
            window_management_key,
            vrr,
            tearing,
            libei,
            ui_drag,
            xwayland,
            color_management,
            float,
            named_actions,
            max_action_depth,
            client_rules,
            window_rules,
            pointer_revert_key,
            use_hardware_cursor: use_hardware_cursor.despan(),
            show_bar: show_bar.despan(),
            show_titles: show_titles.despan(),
            focus_history,
            middle_click_paste: middle_click_paste.despan(),
            input_modes,
            workspace_display_order,
            simple_im,
            fallback_output_mode,
        })
    }
}
