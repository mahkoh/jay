use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::input::FullscreenFocusMove;
use thiserror::Error;

pub struct FullscreenFocusMoveParser;

#[derive(Debug, Error)]
pub enum FullscreenFocusMoveParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown focus move {0}")]
    Unknown(String),
}

impl Parser for FullscreenFocusMoveParser {
    type Value = FullscreenFocusMove;
    type Error = FullscreenFocusMoveParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use FullscreenFocusMove::*;
        let api = match string.to_ascii_lowercase().as_str() {
            "output" => Output,
            "workspace" => Workspace,
            _ => {
                return Err(
                    FullscreenFocusMoveParserError::Unknown(string.to_string()).spanned(span)
                );
            }
        };
        Ok(api)
    }
}
