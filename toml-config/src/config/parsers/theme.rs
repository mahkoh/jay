use crate::config::Theme;
use crate::config::WindowTheme;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::s32;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::color::ColorParser;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::theme::BarPosition;
use jay_config::theme::Color;
use jay_config::theme::ContainerBorders;
use thiserror::Error;

const ATTENTION_REQUESTED_BG_COLOR: &str = "attention-requested-bg-color";
const BG_COLOR: &str = "bg-color";
const BAR_BG_COLOR: &str = "bar-bg-color";
const BAR_STATUS_TEXT_COLOR: &str = "bar-status-text-color";
const BORDER_COLOR: &str = "border-color";
const CAPTURED_FOCUSED_TITLE_BG_COLOR: &str = "captured-focused-title-bg-color";
const CAPTURED_UNFOCUSED_TITLE_BG_COLOR: &str = "captured-unfocused-title-bg-color";
const FOCUSED_INACTIVE_TITLE_BG_COLOR: &str = "focused-inactive-title-bg-color";
const FOCUSED_INACTIVE_TITLE_TEXT_COLOR: &str = "focused-inactive-title-text-color";
const FOCUSED_TITLE_BG_COLOR: &str = "focused-title-bg-color";
const FOCUSED_TITLE_TEXT_COLOR: &str = "focused-title-text-color";
const SEPARATOR_COLOR: &str = "separator-color";
const UNFOCUSED_TITLE_BG_COLOR: &str = "unfocused-title-bg-color";
const UNFOCUSED_TITLE_TEXT_COLOR: &str = "unfocused-title-text-color";
const HIGHLIGHT_COLOR: &str = "highlight-color";
const BORDER_WIDTH: &str = "border-width";
const TITLE_HEIGHT: &str = "title-height";
const BAR_HEIGHT: &str = "bar-height";
const FONT: &str = "font";
const TITLE_FONT: &str = "title-font";
const BAR_FONT: &str = "bar-font";
const BAR_POSITION: &str = "bar-position";
const BAR_SEPARATOR_WIDTH: &str = "bar-separator-width";
const SHOW_WINDOW_ICONS: &str = "show-window-icons";
const WINDOW_ICONS_GRAYSCALE: &str = "window-icons-grayscale";
const CONTAINER_BORDERS: &str = "container-borders";
const FOCUSED_BORDER_COLOR: &str = "focused-border-color";
const SHOW_TITLES: &str = "show-titles";

pub struct ThemeParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

#[derive(Debug, Error)]
pub enum ThemeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
}

impl Parser for ThemeParser<'_, '_, '_> {
    type Value = Theme;
    type Error = ThemeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            (
                attention_requested_bg_color,
                bg_color,
                bar_bg_color,
                bar_status_text_color,
                border_color,
                captured_focused_title_bg_color,
                captured_unfocused_title_bg_color,
                focused_inactive_title_bg_color,
                focused_inactive_title_text_color,
                focused_title_bg_color,
            ),
            (
                focused_title_text_color,
                separator_color,
                unfocused_title_bg_color,
                unfocused_title_text_color,
                highlight_color,
                border_width,
                title_height,
                bar_height,
                font,
                title_font,
            ),
            (
                bar_font,
                bar_position_val,
                bar_separator_width,
                show_window_icons,
                window_icons_grayscale,
                container_borders_val,
                focused_border_color,
            ),
        ) = ext.extract((
            (
                opt(val(ATTENTION_REQUESTED_BG_COLOR)),
                opt(val(BG_COLOR)),
                opt(val(BAR_BG_COLOR)),
                opt(val(BAR_STATUS_TEXT_COLOR)),
                opt(val(BORDER_COLOR)),
                opt(val(CAPTURED_FOCUSED_TITLE_BG_COLOR)),
                opt(val(CAPTURED_UNFOCUSED_TITLE_BG_COLOR)),
                opt(val(FOCUSED_INACTIVE_TITLE_BG_COLOR)),
                opt(val(FOCUSED_INACTIVE_TITLE_TEXT_COLOR)),
                opt(val(FOCUSED_TITLE_BG_COLOR)),
            ),
            (
                opt(val(FOCUSED_TITLE_TEXT_COLOR)),
                opt(val(SEPARATOR_COLOR)),
                opt(val(UNFOCUSED_TITLE_BG_COLOR)),
                opt(val(UNFOCUSED_TITLE_TEXT_COLOR)),
                opt(val(HIGHLIGHT_COLOR)),
                recover(opt(s32(BORDER_WIDTH))),
                recover(opt(s32(TITLE_HEIGHT))),
                recover(opt(s32(BAR_HEIGHT))),
                recover(opt(str(FONT))),
                recover(opt(str(TITLE_FONT))),
            ),
            (
                recover(opt(str(BAR_FONT))),
                recover(opt(str(BAR_POSITION))),
                recover(opt(s32(BAR_SEPARATOR_WIDTH))),
                recover(opt(bol(SHOW_WINDOW_ICONS))),
                recover(opt(bol(WINDOW_ICONS_GRAYSCALE))),
                recover(opt(str(CONTAINER_BORDERS))),
                opt(val(FOCUSED_BORDER_COLOR)),
            ),
        ))?;
        macro_rules! color {
            ($e:expr) => {
                parse_color(self.0, $e)
            };
        }
        let bar_position =
            bar_position_val.and_then(|value| match value.value.to_lowercase().as_str() {
                "top" => Some(BarPosition::Top),
                "bottom" => Some(BarPosition::Bottom),
                _ => {
                    log::warn!(
                        "Unknown bar position '{}': {}",
                        value.value,
                        self.0.error3(value.span)
                    );
                    None
                }
            });
        let container_borders =
            container_borders_val.and_then(|value| parse_container_borders(self.0, value));
        Ok(Theme {
            attention_requested_bg_color: color!(attention_requested_bg_color),
            bg_color: color!(bg_color),
            bar_bg_color: color!(bar_bg_color),
            bar_status_text_color: color!(bar_status_text_color),
            border_color: color!(border_color),
            focused_border_color: color!(focused_border_color),
            captured_focused_title_bg_color: color!(captured_focused_title_bg_color),
            captured_unfocused_title_bg_color: color!(captured_unfocused_title_bg_color),
            focused_inactive_title_bg_color: color!(focused_inactive_title_bg_color),
            focused_inactive_title_text_color: color!(focused_inactive_title_text_color),
            focused_title_bg_color: color!(focused_title_bg_color),
            focused_title_text_color: color!(focused_title_text_color),
            separator_color: color!(separator_color),
            unfocused_title_bg_color: color!(unfocused_title_bg_color),
            unfocused_title_text_color: color!(unfocused_title_text_color),
            highlight_color: color!(highlight_color),
            border_width: border_width.despan(),
            title_height: title_height.despan(),
            bar_height: bar_height.despan(),
            font: font.map(|f| f.value.to_string()),
            title_font: title_font.map(|f| f.value.to_string()),
            bar_font: bar_font.map(|f| f.value.to_string()),
            bar_position,
            bar_separator_width: bar_separator_width.despan(),
            show_window_icons: show_window_icons.despan(),
            window_icons_grayscale: window_icons_grayscale.despan(),
            container_borders,
        })
    }
}

fn parse_color(cx: &Context<'_, '_>, value: Option<Spanned<&Value>>) -> Option<Color> {
    match value?.parse(&mut ColorParser) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("Could not parse a color: {}", cx.error(e));
            None
        }
    }
}

fn parse_container_borders(cx: &Context<'_, '_>, value: Spanned<&str>) -> Option<ContainerBorders> {
    match value.value.to_lowercase().as_str() {
        "separators" => Some(ContainerBorders::Separators),
        "full" => Some(ContainerBorders::Full),
        "full-smart" => Some(ContainerBorders::FullSmart),
        _ => {
            log::warn!(
                "Unknown container borders '{}': {}",
                value.value,
                cx.error3(value.span)
            );
            None
        }
    }
}

pub struct WindowThemeParser<'a, 'b, 'c> {
    pub cx: &'a Context<'b, 'c>,
    pub container: bool,
}

impl Parser for WindowThemeParser<'_, '_, '_> {
    type Value = WindowTheme;
    type Error = ThemeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            (
                attention_requested_bg_color,
                border_color,
                focused_border_color,
                focused_inactive_title_bg_color,
                focused_inactive_title_text_color,
                focused_title_bg_color,
                focused_title_text_color,
                separator_color,
                unfocused_title_bg_color,
                unfocused_title_text_color,
            ),
            (
                border_width,
                title_height,
                title_font,
                show_titles,
                show_window_icons,
                window_icons_grayscale,
            ),
        ) = ext.extract((
            (
                opt(val(ATTENTION_REQUESTED_BG_COLOR)),
                opt(val(BORDER_COLOR)),
                opt(val(FOCUSED_BORDER_COLOR)),
                opt(val(FOCUSED_INACTIVE_TITLE_BG_COLOR)),
                opt(val(FOCUSED_INACTIVE_TITLE_TEXT_COLOR)),
                opt(val(FOCUSED_TITLE_BG_COLOR)),
                opt(val(FOCUSED_TITLE_TEXT_COLOR)),
                opt(val(SEPARATOR_COLOR)),
                opt(val(UNFOCUSED_TITLE_BG_COLOR)),
                opt(val(UNFOCUSED_TITLE_TEXT_COLOR)),
            ),
            (
                recover(opt(s32(BORDER_WIDTH))),
                recover(opt(s32(TITLE_HEIGHT))),
                recover(opt(str(TITLE_FONT))),
                recover(opt(bol(SHOW_TITLES))),
                recover(opt(bol(SHOW_WINDOW_ICONS))),
                recover(opt(bol(WINDOW_ICONS_GRAYSCALE))),
            ),
        ))?;
        let mut container_borders = None;
        if self.container {
            container_borders = ext
                .extract(recover(opt(str(CONTAINER_BORDERS))))?
                .and_then(|value| parse_container_borders(self.cx, value));
        }
        macro_rules! color {
            ($e:expr) => {
                parse_color(self.cx, $e)
            };
        }
        Ok(WindowTheme {
            attention_requested_bg_color: color!(attention_requested_bg_color),
            border_color: color!(border_color),
            focused_border_color: color!(focused_border_color),
            focused_inactive_title_bg_color: color!(focused_inactive_title_bg_color),
            focused_inactive_title_text_color: color!(focused_inactive_title_text_color),
            focused_title_bg_color: color!(focused_title_bg_color),
            focused_title_text_color: color!(focused_title_text_color),
            separator_color: color!(separator_color),
            unfocused_title_bg_color: color!(unfocused_title_bg_color),
            unfocused_title_text_color: color!(unfocused_title_text_color),
            border_width: border_width.despan(),
            title_height: title_height.despan(),
            title_font: title_font.map(|f| f.value.to_string()),
            show_titles: show_titles.despan(),
            show_window_icons: show_window_icons.despan(),
            window_icons_grayscale: window_icons_grayscale.despan(),
            container_borders,
        })
    }
}
