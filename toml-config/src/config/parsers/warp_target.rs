use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::input::WarpTarget;
use thiserror::Error;

pub struct WarpTargetParser;

#[derive(Debug, Error)]
pub enum WarpTargetParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown warp target {0}")]
    Unknown(String),
}

impl Parser for WarpTargetParser {
    type Value = WarpTarget;
    type Error = WarpTargetParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let target = match string.to_ascii_lowercase().as_str() {
            "window" => WarpTarget::Window,
            "workspace" => WarpTarget::Workspace,
            "output" => WarpTarget::Output,
            _ => return Err(WarpTargetParserError::Unknown(string.to_string()).spanned(span)),
        };
        Ok(target)
    }
}
