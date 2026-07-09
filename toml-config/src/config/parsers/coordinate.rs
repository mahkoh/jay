use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::window::Coordinate;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoordinateParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Expected the string \"keep\" but found \"{0}\"")]
    UnknownKeyword(String),
    #[error("Value must fit in a i32")]
    NotI32,
}

pub struct CoordinateParser;

impl Parser for CoordinateParser {
    type Value = Coordinate;
    type Error = CoordinateParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Integer];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        match string {
            "keep" => Ok(Coordinate::Keep),
            _ => Err(CoordinateParserError::UnknownKeyword(string.to_string()).spanned(span)),
        }
    }

    fn parse_integer(&mut self, span: Span, integer: i64) -> ParseResult<Self> {
        match i32::try_from(integer) {
            Ok(n) => Ok(Coordinate::Set(n)),
            Err(_) => Err(CoordinateParserError::NotI32.spanned(span)),
        }
    }
}
