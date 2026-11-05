use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::FlattenTree;
use thiserror::Error;

pub struct FlattenTreeParser;

#[derive(Debug, Error)]
pub enum FlattenTreeParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown flatten-tree value {0}")]
    Unknown(String),
}

impl Parser for FlattenTreeParser {
    type Value = FlattenTree;
    type Error = FlattenTreeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use FlattenTree::*;
        let mode = match string.to_ascii_lowercase().as_str() {
            "always" => Always,
            "on-remove" => OnRemove,
            "never" => Never,
            _ => return Err(FlattenTreeParserError::Unknown(string.to_string()).spanned(span)),
        };
        Ok(mode)
    }
}
