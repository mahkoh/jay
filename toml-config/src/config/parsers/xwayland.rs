use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            Xwayland,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::xwayland::XScalingMode,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum XwaylandParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct XwaylandParser<'a>(pub &'a Context<'a>);

impl Parser for XwaylandParser<'_> {
    type Value = Xwayland;
    type Error = XwaylandParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let scaling_mode = ext.extract(opt(val("scaling-mode")))?;
        let scaling_mode = scaling_mode.and_then(|m| match m.parse(&mut XScalingModeParser) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("Could not parse scaling mode: {}", self.0.error(e));
                None
            }
        });
        Ok(Xwayland { scaling_mode })
    }
}

#[derive(Debug, Error)]
pub enum XScalingModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown mode {0}")]
    UnknownMode(String),
}

struct XScalingModeParser;

impl Parser for XScalingModeParser {
    type Value = XScalingMode;
    type Error = XScalingModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mode = match string {
            "default" => XScalingMode::DEFAULT,
            "downscaled" => XScalingMode::DOWNSCALED,
            _ => return Err(XScalingModeParserError::UnknownMode(string.to_string()).spanned(span)),
        };
        Ok(mode)
    }
}
