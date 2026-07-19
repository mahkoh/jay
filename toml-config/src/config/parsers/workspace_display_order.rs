use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::workspace::WorkspaceDisplayOrder;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceDisplayOrderParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown workspace display order {0}")]
    Unknown(String),
}

pub struct WorkspaceDisplayOrderParser;

impl Parser for WorkspaceDisplayOrderParser {
    type Value = WorkspaceDisplayOrder;
    type Error = WorkspaceDisplayOrderParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        match string {
            "manual" => Ok(WorkspaceDisplayOrder::Manual),
            "sorted" => Ok(WorkspaceDisplayOrder::Sorted),
            _ => Err(WorkspaceDisplayOrderParserError::Unknown(string.to_string()).spanned(span)),
        }
    }
}
