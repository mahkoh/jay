use {
    crate::{
        config::{
            context::Context,
            extractor::ExtractorError,
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::theme::Color,
    std::{num::ParseIntError, ops::Range},
    thiserror::Error,
};

pub struct ColorParser<'a>(pub &'a Context<'a>);

#[derive(Debug, Error)]
pub enum ColorParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
    #[error("Color must start with `#`")]
    Prefix,
    #[error("String must have length 4, 5, 6, or 9")]
    Length,
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
}

impl Parser for ColorParser<'_> {
    type Value = Color;
    type Error = ColorParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let hex = match string.strip_prefix("#") {
            Some(s) => s,
            _ => return Err(ColorParserError::Prefix.spanned(span)),
        };
        let d = |range: Range<usize>| {
            u8::from_str_radix(&hex[range], 16)
                .map_err(|e| ColorParserError::ParseIntError(e).spanned(span))
        };
        let s = |range: Range<usize>| {
            let v = d(range)?;
            Ok((v << 4) | v)
        };
        let (r, g, b, a) = match hex.len() {
            3 => (s(0..1)?, s(1..2)?, s(2..3)?, u8::MAX),
            4 => (s(0..1)?, s(1..2)?, s(2..3)?, s(3..4)?),
            6 => (d(0..2)?, d(2..4)?, d(4..6)?, u8::MAX),
            8 => (d(0..2)?, d(2..4)?, d(4..6)?, d(4..8)?),
            _ => return Err(ColorParserError::Length.spanned(span)),
        };
        Ok(Color::new_straight(r, g, b, a))
    }
}
