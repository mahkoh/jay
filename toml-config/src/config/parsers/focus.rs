use crate::config::Direction;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::str;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use jay_config::focus;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FocusParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Unknown focus type {0}")]
    UnknownKind(String),
    #[error("Unknown edge mode {0}")]
    UnknownMode(String),
}

pub struct FocusTargetParser<'a, 'b, 'c> {
    pub cx: &'a Context<'b, 'c>,
    pub default_edge_mode: focus::EdgeMode,
    pub direction: Direction,
}

impl FocusTargetParser<'_, '_, '_> {
    fn parse_kind(
        &mut self,
        span: Span,
        string: &str,
    ) -> Result<focus::Kind, Spanned<FocusParserError>> {
        match string.to_ascii_lowercase().as_str() {
            "split" => Ok(focus::Kind::Split),
            "mono" => Ok(focus::Kind::Mono),
            "float" => Ok(focus::Kind::Float),
            "workspace" => Ok(focus::Kind::Workspace),
            "output" => Ok(focus::Kind::Output),
            _ => Err(FocusParserError::UnknownKind(string.to_string()).spanned(span)),
        }
    }
}

// Convert from Direction to (backward, vertical) pair.
fn map_direction(direction: Direction) -> (bool, bool) {
    match direction {
        Direction::Up => (true, true),
        Direction::Down => (false, true),
        Direction::Left => (true, false),
        Direction::Right => (false, false),
    }
}

impl Parser for FocusTargetParser<'_, '_, '_> {
    type Value = focus::Target;

    type Error = FocusParserError;

    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Table];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let kind = self.parse_kind(span, string)?;
        let (backward, vertical) = map_direction(self.direction);
        Ok(focus::Target {
            kind,
            backward,
            vertical,
            edge_mode: self.default_edge_mode,
        })
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &indexmap::IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        // TODO: make edge_mode optional.
        let (kind_val, edge_mode_val) = ext.extract((str("type"), str("edge_mode")))?;

        let kind = self.parse_kind(kind_val.span, kind_val.value)?;
        let (backward, vertical) = map_direction(self.direction);
        let edge_mode = match edge_mode_val.value.to_ascii_lowercase().as_str() {
            "stop" => focus::EdgeMode::Stop,
            "wrap" => focus::EdgeMode::Wrap,
            "traverse" => focus::EdgeMode::Traverse,
            "inactive" => focus::EdgeMode::Inactive,
            _ => {
                return Err(
                    FocusParserError::UnknownMode(edge_mode_val.value.to_string()).spanned(span),
                );
            }
        };

        Ok(focus::Target {
            kind,
            backward,
            vertical,
            edge_mode,
        })
    }
}
