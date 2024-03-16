use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, recover, s32, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::color::ColorParser,
            Theme,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

pub struct ThemeParser<'a>(pub &'a Context<'a>);

#[derive(Debug, Error)]
pub enum ThemeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
}

impl Parser for ThemeParser<'_> {
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
                border_width,
                title_height,
                font,
            ),
        ) = ext.extract((
            (
                opt(val("attention-requested-bg-color")),
                opt(val("bg-color")),
                opt(val("bar-bg-color")),
                opt(val("bar-status-text-color")),
                opt(val("border-color")),
                opt(val("captured-focused-title-bg-color")),
                opt(val("captured-unfocused-title-bg-color")),
                opt(val("focused-inactive-title-bg-color")),
                opt(val("focused-inactive-title-text-color")),
                opt(val("focused-title-bg-color")),
            ),
            (
                opt(val("focused-title-text-color")),
                opt(val("separator-color")),
                opt(val("unfocused-title-bg-color")),
                opt(val("unfocused-title-text-color")),
                recover(opt(s32("border-width"))),
                recover(opt(s32("title-height"))),
                recover(opt(str("font"))),
            ),
        ))?;
        macro_rules! color {
            ($e:expr) => {
                match $e {
                    None => None,
                    Some(v) => match v.parse(&mut ColorParser(self.0)) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            log::warn!("Could not parse a color: {}", self.0.error(e));
                            None
                        }
                    },
                }
            };
        }
        Ok(Theme {
            attention_requested_bg_color: color!(attention_requested_bg_color),
            bg_color: color!(bg_color),
            bar_bg_color: color!(bar_bg_color),
            bar_status_text_color: color!(bar_status_text_color),
            border_color: color!(border_color),
            captured_focused_title_bg_color: color!(captured_focused_title_bg_color),
            captured_unfocused_title_bg_color: color!(captured_unfocused_title_bg_color),
            focused_inactive_title_bg_color: color!(focused_inactive_title_bg_color),
            focused_inactive_title_text_color: color!(focused_inactive_title_text_color),
            focused_title_bg_color: color!(focused_title_bg_color),
            focused_title_text_color: color!(focused_title_text_color),
            separator_color: color!(separator_color),
            unfocused_title_bg_color: color!(unfocused_title_bg_color),
            unfocused_title_text_color: color!(unfocused_title_text_color),
            border_width: border_width.despan(),
            title_height: title_height.despan(),
            font: font.map(|f| f.value.to_string()),
        })
    }
}
