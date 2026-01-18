use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::input::FallbackOutputMode,
    thiserror::Error,
};

pub struct FallbackOutputModeParser;

#[derive(Debug, Error)]
pub enum FallbackOutputModeParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown mode {0}")]
    Unknown(String),
}

impl Parser for FallbackOutputModeParser {
    type Value = FallbackOutputMode;
    type Error = FallbackOutputModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use FallbackOutputMode::*;
        let api = match string.to_ascii_lowercase().as_str() {
            "cursor" => Cursor,
            "focus" => Focus,
            _ => {
                return Err(
                    FallbackOutputModeParserError::Unknown(string.to_string()).spanned(span)
                );
            }
        };
        Ok(api)
    }
}
