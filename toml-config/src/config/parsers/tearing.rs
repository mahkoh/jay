use crate::config::Tearing;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::video::TearingMode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TearingParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct TearingParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for TearingParser<'_, '_> {
    type Value = Tearing;
    type Error = TearingParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let mode = ext.extract(opt(val("mode")))?;
        let mode = mode.and_then(|m| match m.parse(&mut TearingModeParser) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("Could not parse mode: {}", self.0.error(e));
                None
            }
        });
        Ok(Tearing { mode })
    }
}

#[derive(Debug, Error)]
pub enum TearingModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown mode {0}")]
    UnknownMode(String),
}

struct TearingModeParser;

impl Parser for TearingModeParser {
    type Value = TearingMode;
    type Error = TearingModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mode = match string {
            "never" => TearingMode::NEVER,
            "always" => TearingMode::ALWAYS,
            "variant1" => TearingMode::VARIANT_1,
            "variant2" => TearingMode::VARIANT_2,
            "variant3" => TearingMode::VARIANT_3,
            _ => return Err(TearingModeParserError::UnknownMode(string.to_string()).spanned(span)),
        };
        Ok(mode)
    }
}
