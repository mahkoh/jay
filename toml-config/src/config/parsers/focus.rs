use crate::config::Direction;
use crate::config::extractor::ExtractorError;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::Axis;
use jay_config::focus::EdgeMode;
use jay_config::focus::Kind;
use jay_config::focus::Target;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FocusTargetParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Unknown focus type {0}")]
    UnknownKind(String),
}

pub struct FocusTargetParser {
    pub edge_mode: EdgeMode,
    pub direction: Direction,
    pub axis: Axis,
    pub backward: bool,
}

impl Parser for FocusTargetParser {
    type Value = Target;

    type Error = FocusTargetParserError;

    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let kind = match string.to_ascii_lowercase().as_str() {
            "split" => Kind::Split(self.axis),
            "mono" => Kind::Mono,
            "float" => Kind::Float,
            "workspace" => Kind::Workspace,
            "output" => Kind::Output(self.direction),
            _ => return Err(FocusTargetParserError::UnknownKind(string.to_string()).spanned(span)),
        };
        Ok(Target {
            kind,
            backward: self.backward,
            edge_mode: self.edge_mode,
        })
    }
}
