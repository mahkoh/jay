use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            Vrr,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::video::VrrMode,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum VrrParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct VrrParser<'a>(pub &'a Context<'a>);

impl Parser for VrrParser<'_> {
    type Value = Vrr;
    type Error = VrrParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (mode, cursor_hz) = ext.extract((opt(val("mode")), opt(val("cursor-hz"))))?;
        let mode = mode.and_then(|m| match m.parse(&mut VrrModeParser) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("Could not parse mode: {}", self.0.error(e));
                None
            }
        });
        let cursor_hz = cursor_hz.and_then(|m| match m.parse(&mut VrrRateParser) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("Could not parse rate: {}", self.0.error(e));
                None
            }
        });
        Ok(Vrr { mode, cursor_hz })
    }
}

#[derive(Debug, Error)]
pub enum VrrModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown mode {0}")]
    UnknownMode(String),
}

struct VrrModeParser;

impl Parser for VrrModeParser {
    type Value = VrrMode;
    type Error = VrrModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mode = match string {
            "never" => VrrMode::NEVER,
            "always" => VrrMode::ALWAYS,
            "variant1" => VrrMode::VARIANT_1,
            "variant2" => VrrMode::VARIANT_2,
            "variant3" => VrrMode::VARIANT_3,
            _ => return Err(VrrModeParserError::UnknownMode(string.to_string()).spanned(span)),
        };
        Ok(mode)
    }
}

#[derive(Debug, Error)]
pub enum VrrRateParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown rate {0}")]
    UnknownString(String),
}

struct VrrRateParser;

impl Parser for VrrRateParser {
    type Value = f64;
    type Error = VrrRateParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Float, DataType::Integer];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        match string {
            "none" => Ok(f64::INFINITY),
            _ => Err(VrrRateParserError::UnknownString(string.to_string()).spanned(span)),
        }
    }

    fn parse_integer(&mut self, _span: Span, integer: i64) -> ParseResult<Self> {
        Ok(integer as _)
    }

    fn parse_float(&mut self, _span: Span, float: f64) -> ParseResult<Self> {
        Ok(float)
    }
}
