use crate::config::Xwayland;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::xwayland::XScalingMode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XwaylandParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct XwaylandParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for XwaylandParser<'_, '_> {
    type Value = Xwayland;
    type Error = XwaylandParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (enabled, scaling_mode) =
            ext.extract((recover(opt(bol("enabled"))), opt(val("scaling-mode"))))?;
        let scaling_mode = scaling_mode.and_then(|m| match m.parse(&mut XScalingModeParser) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("Could not parse scaling mode: {}", self.0.error(e));
                None
            }
        });
        Ok(Xwayland {
            enabled: enabled.despan(),
            scaling_mode,
        })
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
