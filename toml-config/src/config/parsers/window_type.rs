use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use jay_config::window;
use jay_config::window::WindowType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowTypeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown window type `{}`", .0)]
    UnknownWindowType(String),
}

pub struct WindowTypeParser;

impl Parser for WindowTypeParser {
    type Value = WindowType;
    type Error = WindowTypeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array, DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let ty = match string {
            "none" => WindowType(0),
            "any" => WindowType(!0),
            "container" => window::CONTAINER,
            "placeholder" => window::PLACEHOLDER,
            "xdg-toplevel" => window::XDG_TOPLEVEL,
            "x-window" => window::X_WINDOW,
            "client-window" => window::CLIENT_WINDOW,
            _ => {
                return Err(
                    WindowTypeParserError::UnknownWindowType(string.to_owned()).spanned(span)
                );
            }
        };
        Ok(ty)
    }

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut ty = WindowType(0);
        for el in array {
            ty |= el.parse(&mut WindowTypeParser)?;
        }
        Ok(ty)
    }
}
