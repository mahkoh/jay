use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::workspace::WorkspaceEmptyBehavior,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum WorkspaceEmptyBehaviorParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown workspace empty behavior {0}")]
    Unknown(String),
}

pub struct WorkspaceEmptyBehaviorParser;

impl Parser for WorkspaceEmptyBehaviorParser {
    type Value = WorkspaceEmptyBehavior;
    type Error = WorkspaceEmptyBehaviorParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        match string {
            "preserve" => Ok(WorkspaceEmptyBehavior::Preserve),
            "destroy-on-leave" => Ok(WorkspaceEmptyBehavior::DestroyOnLeave),
            "hide-on-leave" => Ok(WorkspaceEmptyBehavior::HideOnLeave),
            "destroy" => Ok(WorkspaceEmptyBehavior::Destroy),
            "hide" => Ok(WorkspaceEmptyBehavior::Hide),
            _ => Err(WorkspaceEmptyBehaviorParserError::Unknown(string.to_string()).spanned(span)),
        }
    }
}
