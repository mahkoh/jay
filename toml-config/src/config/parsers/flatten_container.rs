use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::FlattenContainer;
use thiserror::Error;

pub struct FlattenContainerParser;

#[derive(Debug, Error)]
pub enum FlattenContainerParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown flatten-container value {0}")]
    Unknown(String),
}

impl Parser for FlattenContainerParser {
    type Value = FlattenContainer;
    type Error = FlattenContainerParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use FlattenContainer::*;
        let mode = match string.to_ascii_lowercase().as_str() {
            "on-remove" => OnRemove,
            "never" => Never,
            _ => return Err(FlattenContainerParserError::Unknown(string.to_string()).spanned(span)),
        };
        Ok(mode)
    }
}
