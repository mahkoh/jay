use {
    crate::{
        config::{
            context::Context,
            extractor::{arr, bol, n32, opt, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                connector::{ConnectorParser, ConnectorParserError},
                drm_device::{DrmDeviceParser, DrmDeviceParserError},
                drm_device_match::{DrmDeviceMatchParser, DrmDeviceMatchParserError},
                env::{EnvParser, EnvParserError},
                exec::{ExecParser, ExecParserError},
                gfx_api::{GfxApiParser, GfxApiParserError},
                idle::{IdleParser, IdleParserError},
                input::{InputParser, InputParserError},
                keymap::{KeymapParser, KeymapParserError},
                log_level::{LogLevelParser, LogLevelParserError},
                output::{OutputParser, OutputParserError},
                output_match::{OutputMatchParser, OutputMatchParserError},
                repeat_rate::{RepeatRateParser, RepeatRateParserError},
                status::{StatusParser, StatusParserError},
                theme::{ThemeParser, ThemeParserError},
                StringParser, StringParserError,
            },
            spanned::SpannedErrorExt,
            Action,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::{
        get_workspace,
        keyboard::AppMod,
        Axis::{Horizontal, Vertical},
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ActionParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    StringParser(#[from] StringParserError),
    #[error("Unknown type {0}")]
    UnknownType(String),
    #[error("Unknown simple action {0}")]
    UnknownSimpleAction(String),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Could not parse the exec action")]
    Exec(#[source] ExecParserError),
    #[error("Could not parse the configure-connector action")]
    ConfigureConnector(#[source] ConnectorParserError),
    #[error("Could not parse the configure-input action")]
    ConfigureInput(#[source] InputParserError),
    #[error("Could not parse the configure-output action")]
    ConfigureOutput(#[source] OutputParserError),
    #[error("Could not parse the environment variables")]
    Env(#[source] EnvParserError),
    #[error("Could not parse a set-keymap action")]
    SetKeymap(#[source] KeymapParserError),
    #[error("Could not parse a set-status action")]
    Status(#[source] StatusParserError),
    #[error("Could not parse a set-theme action")]
    Theme(#[source] ThemeParserError),
    #[error("Could not parse a set-log-level action")]
    SetLogLevel(#[source] LogLevelParserError),
    #[error("Could not parse a set-gfx-api action")]
    GfxApi(#[source] GfxApiParserError),
    #[error("Could not parse a configure-drm-device action")]
    DrmDevice(#[source] DrmDeviceParserError),
    #[error("Could not parse a set-render-device action")]
    SetRenderDevice(#[source] DrmDeviceMatchParserError),
    #[error("Could not parse a configure-idle action")]
    ConfigureIdle(#[source] IdleParserError),
    #[error("Could not parse a move-to-output action")]
    MoveToOutput(#[source] OutputMatchParserError),
    #[error("Could not parse a set-repeat-rate action")]
    RepeatRate(#[source] RepeatRateParserError),
    #[error("Could not parse set_app_mod : too many args, 2 expected.")]
    SetAppModTooManyArgs,
}

pub struct ActionParser<'a>(pub &'a Context<'a>);

impl ActionParser<'_> {
    fn parse_simple_cmd(&self, span: Span, string: &str) -> ParseResult<Self> {
        use {crate::config::SimpleCommand::*, jay_config::Direction::*};
        let cmd = match string {
            "focus-left" => Focus(Left),
            "focus-down" => Focus(Down),
            "focus-up" => Focus(Up),
            "focus-right" => Focus(Right),
            "move-left" => Move(Left),
            "move-down" => Move(Down),
            "move-up" => Move(Up),
            "move-right" => Move(Right),
            "split-horizontal" => Split(Horizontal),
            "split-vertical" => Split(Vertical),
            "toggle-split" => ToggleSplit,
            "toggle-mono" => ToggleMono,
            "toggle-fullscreen" => ToggleFullscreen,
            "focus-parent" => FocusParent,
            "close" => Close,
            "disable-pointer-constraint" => DisablePointerConstraint,
            "toggle-floating" => ToggleFloating,
            "quit" => Quit,
            "reload-config-toml" => ReloadConfigToml,
            "reload-config-so" => ReloadConfigSo,
            "none" => None,
            "forward" => Forward(true),
            "consume" => Forward(false),
            "enable-window-management" => EnableWindowManagement(true),
            "disable-window-management" => EnableWindowManagement(false),
            string => {
                if string.starts_with("set_app_mod(") {
                    static FN_PART_LEN: usize = 12;
                    let len = string.len();
                    // remove function name and first parenthesis
                    let string = string.chars().skip(FN_PART_LEN);
                    // remove last parenthesis
                    let string = string.take(len - FN_PART_LEN - 1).collect::<String>();
                    let mut args: Vec<_> = string.split(',').collect();
                    if args.len() != 2 {
                        return Err(ActionParserError::SetAppModTooManyArgs.spanned(span));
                    }
                    fn skip_spaces(arg: impl IntoIterator<Item = char>) -> String {
                        arg.into_iter().skip_while(|c| c == &' ').collect()
                    }
                    fn parse_arg(arg: &str) -> String {
                        let skipped_beg = skip_spaces(arg.chars());
                        let skipped_end = skip_spaces(skipped_beg.chars().rev());
                        skipped_end.chars().rev().collect()
                    }
                    let mod_name = parse_arg(args.pop().unwrap());
                    let app_name = parse_arg(args.pop().unwrap());
                    let app_mod = AppMod { app_name, mod_name };
                    SetAppMod(app_mod)
                } else {
                    return Err(
                        ActionParserError::UnknownSimpleAction(string.to_string()).spanned(span)
                    );
                }
            }
        };
        Ok(Action::SimpleCommand { cmd })
    }

    fn parse_multi(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut actions = vec![];
        for el in array {
            actions.push(el.parse(self)?);
        }
        Ok(Action::Multi { actions })
    }

    fn parse_exec(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let exec = ext
            .extract(val("exec"))?
            .parse_map(&mut ExecParser(self.0))
            .map_spanned_err(ActionParserError::Exec)?;
        Ok(Action::Exec { exec })
    }

    fn parse_switch_to_vt(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let num = ext.extract(n32("num"))?.value;
        Ok(Action::SwitchToVt { num })
    }

    fn parse_show_workspace(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let name = ext.extract(str("name"))?.value.to_string();
        Ok(Action::ShowWorkspace { name })
    }

    fn parse_move_to_workspace(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let name = ext.extract(str("name"))?.value.to_string();
        Ok(Action::MoveToWorkspace { name })
    }

    fn parse_configure_connector(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let con = ext
            .extract(val("connector"))?
            .parse_map(&mut ConnectorParser(self.0))
            .map_spanned_err(ActionParserError::ConfigureConnector)?;
        Ok(Action::ConfigureConnector { con })
    }

    fn parse_configure_input(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let input = ext
            .extract(val("input"))?
            .parse_map(&mut InputParser {
                cx: self.0,
                is_inputs_array: false,
            })
            .map_spanned_err(ActionParserError::ConfigureInput)?;
        Ok(Action::ConfigureInput {
            input: Box::new(input),
        })
    }

    fn parse_configure_idle(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let idle = ext
            .extract(val("idle"))?
            .parse_map(&mut IdleParser(self.0))
            .map_spanned_err(ActionParserError::ConfigureIdle)?;
        Ok(Action::ConfigureIdle { idle })
    }

    fn parse_configure_output(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let out = ext
            .extract(val("output"))?
            .parse_map(&mut OutputParser {
                cx: self.0,
                name_ok: false,
            })
            .map_spanned_err(ActionParserError::ConfigureOutput)?;
        Ok(Action::ConfigureOutput { out })
    }

    fn parse_set_env(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let env = ext
            .extract(val("env"))?
            .parse_map(&mut EnvParser)
            .map_spanned_err(ActionParserError::Env)?;
        Ok(Action::SetEnv { env })
    }

    fn parse_unset_env(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        struct P;
        impl Parser for P {
            type Value = Vec<String>;
            type Error = ActionParserError;
            const EXPECTED: &'static [DataType] = &[DataType::Array, DataType::String];

            fn parse_string(&mut self, _span: Span, string: &str) -> ParseResult<Self> {
                Ok(vec![string.to_string()])
            }

            fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
                let mut res = vec![];
                for v in array {
                    res.push(v.parse_map(&mut StringParser)?);
                }
                Ok(res)
            }
        }
        let env = ext.extract(val("env"))?.parse_map(&mut P)?;
        Ok(Action::UnsetEnv { env })
    }

    fn parse_set_keymap(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let map = ext
            .extract(val("map"))?
            .parse_map(&mut KeymapParser {
                cx: self.0,
                definition: false,
            })
            .map_spanned_err(ActionParserError::SetKeymap)?;
        Ok(Action::SetKeymap { map })
    }

    fn parse_set_status(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let status = match ext.extract(opt(val("status")))? {
            None => None,
            Some(v) => Some(
                v.parse_map(&mut StatusParser(self.0))
                    .map_spanned_err(ActionParserError::Status)?,
            ),
        };
        Ok(Action::SetStatus { status })
    }

    fn parse_set_theme(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let theme = ext
            .extract(val("theme"))?
            .parse_map(&mut ThemeParser(self.0))
            .map_spanned_err(ActionParserError::Theme)?;
        Ok(Action::SetTheme {
            theme: Box::new(theme),
        })
    }

    fn parse_set_log_level(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let level = ext
            .extract(val("level"))?
            .parse_map(&mut LogLevelParser)
            .map_spanned_err(ActionParserError::SetLogLevel)?;
        Ok(Action::SetLogLevel { level })
    }

    fn parse_set_gfx_api(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let api = ext
            .extract(val("api"))?
            .parse_map(&mut GfxApiParser)
            .map_spanned_err(ActionParserError::GfxApi)?;
        Ok(Action::SetGfxApi { api })
    }

    fn parse_set_render_device(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let dev = ext
            .extract(val("dev"))?
            .parse_map(&mut DrmDeviceMatchParser(self.0))
            .map_spanned_err(ActionParserError::SetRenderDevice)?;
        Ok(Action::SetRenderDevice { dev: Box::new(dev) })
    }

    fn parse_configure_direct_scanout(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let enabled = ext.extract(bol("enabled"))?.value;
        Ok(Action::ConfigureDirectScanout { enabled })
    }

    fn parse_configure_drm_device(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let dev = ext
            .extract(val("dev"))?
            .parse_map(&mut DrmDeviceParser {
                cx: self.0,
                name_ok: false,
            })
            .map_spanned_err(ActionParserError::DrmDevice)?;
        Ok(Action::ConfigureDrmDevice { dev })
    }

    fn parse_move_to_output(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let (ws, output) = ext.extract((opt(str("workspace")), val("output")))?;
        let output = output
            .parse_map(&mut OutputMatchParser(self.0))
            .map_spanned_err(ActionParserError::MoveToOutput)?;
        Ok(Action::MoveToOutput {
            workspace: ws.despan().map(get_workspace),
            output,
        })
    }

    fn parse_set_repeat_rate(&mut self, ext: &mut Extractor<'_>) -> ParseResult<Self> {
        let rate = ext
            .extract(val("rate"))?
            .parse_map(&mut RepeatRateParser(self.0))
            .map_spanned_err(ActionParserError::RepeatRate)?;
        Ok(Action::SetRepeatRate { rate })
    }
}

impl Parser for ActionParser<'_> {
    type Value = Action;
    type Error = ActionParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Array, DataType::Table];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        self.parse_simple_cmd(span, string)
    }

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        self.parse_multi(span, array)
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let ty = ext.extract_or_ignore(str("type"))?;
        let res = match ty.value {
            "simple" => {
                let cmd = ext.extract(str("cmd"))?;
                self.parse_simple_cmd(cmd.span, cmd.value)
            }
            "multi" => {
                let actions = ext.extract(arr("actions"))?;
                self.parse_multi(actions.span, actions.value)
            }
            "exec" => self.parse_exec(&mut ext),
            "switch-to-vt" => self.parse_switch_to_vt(&mut ext),
            "show-workspace" => self.parse_show_workspace(&mut ext),
            "move-to-workspace" => self.parse_move_to_workspace(&mut ext),
            "configure-connector" => self.parse_configure_connector(&mut ext),
            "configure-input" => self.parse_configure_input(&mut ext),
            "configure-output" => self.parse_configure_output(&mut ext),
            "set-env" => self.parse_set_env(&mut ext),
            "unset-env" => self.parse_unset_env(&mut ext),
            "set-keymap" => self.parse_set_keymap(&mut ext),
            "set-status" => self.parse_set_status(&mut ext),
            "set-theme" => self.parse_set_theme(&mut ext),
            "set-log-level" => self.parse_set_log_level(&mut ext),
            "set-gfx-api" => self.parse_set_gfx_api(&mut ext),
            "configure-direct-scanout" => self.parse_configure_direct_scanout(&mut ext),
            "configure-drm-device" => self.parse_configure_drm_device(&mut ext),
            "set-render-device" => self.parse_set_render_device(&mut ext),
            "configure-idle" => self.parse_configure_idle(&mut ext),
            "move-to-output" => self.parse_move_to_output(&mut ext),
            "set-repeat-rate" => self.parse_set_repeat_rate(&mut ext),
            v => {
                ext.ignore_unused();
                return Err(ActionParserError::UnknownType(v.to_string()).spanned(ty.span));
            }
        };
        drop(ext);
        res
    }
}
