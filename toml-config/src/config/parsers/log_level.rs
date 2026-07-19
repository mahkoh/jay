use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::logging::LogLevel;
use thiserror::Error;

pub struct LogLevelParser;

#[derive(Debug, Error)]
pub enum LogLevelParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown log level {0}")]
    Unknown(String),
}

impl Parser for LogLevelParser {
    type Value = LogLevel;
    type Error = LogLevelParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use LogLevel::*;
        let level = match string.to_ascii_lowercase().as_str() {
            "error" => Error,
            "warn" | "warning" => Warn,
            "info" => Info,
            "debug" => Debug,
            "trace" => Trace,
            _ => return Err(LogLevelParserError::Unknown(string.to_string()).spanned(span)),
        };
        Ok(level)
    }
}
