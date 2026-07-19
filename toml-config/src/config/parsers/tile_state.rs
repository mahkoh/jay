use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::window::TileState;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TileStateParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown tile state `{}`", .0)]
    UnknownTileState(String),
}

pub struct TileStateParser;

impl Parser for TileStateParser {
    type Value = TileState;
    type Error = TileStateParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let ty = match string {
            "tiled" => TileState::Tiled,
            "floating" => TileState::Floating,
            _ => {
                return Err(TileStateParserError::UnknownTileState(string.to_owned()).spanned(span));
            }
        };
        Ok(ty)
    }
}
