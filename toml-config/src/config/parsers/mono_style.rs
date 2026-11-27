use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::window::MonoStyle;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MonoStyleParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown mono style `{}`", .0)]
    UnknownMonoStyle(String),
}

pub struct MonoStyleParser;

impl Parser for MonoStyleParser {
    type Value = MonoStyle;
    type Error = MonoStyleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let ty = match string {
            "tabbed" => MonoStyle::Tabbed,
            "stacked" => MonoStyle::Stacked,
            _ => {
                return Err(MonoStyleParserError::UnknownMonoStyle(string.to_owned()).spanned(span));
            }
        };
        Ok(ty)
    }
}
