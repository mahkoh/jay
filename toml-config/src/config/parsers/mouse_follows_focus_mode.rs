use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::input::MouseFollowsFocusMode;
use thiserror::Error;

pub struct MouseFollowsFocusModeParser;

#[derive(Debug, Error)]
pub enum MouseFollowsFocusModeParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown mode {0}")]
    Unknown(String),
}

impl Parser for MouseFollowsFocusModeParser {
    type Value = MouseFollowsFocusMode;
    type Error = MouseFollowsFocusModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Boolean, DataType::String];

    fn parse_bool(&mut self, _span: Span, bool: bool) -> ParseResult<Self> {
        let mode = match bool {
            true => MouseFollowsFocusMode::Window,
            false => MouseFollowsFocusMode::None,
        };
        Ok(mode)
    }

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mode = match string.to_ascii_lowercase().as_str() {
            "none" => MouseFollowsFocusMode::None,
            "output" => MouseFollowsFocusMode::Output,
            "workspace" => MouseFollowsFocusMode::Workspace,
            "window" => MouseFollowsFocusMode::Window,
            _ => {
                return Err(
                    MouseFollowsFocusModeParserError::Unknown(string.to_string()).spanned(span),
                );
            }
        };
        Ok(mode)
    }
}
