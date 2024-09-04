use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::video::Format,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum FormatParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown format {0}")]
    UnknownFormat(String),
}

pub struct FormatParser;

impl Parser for FormatParser {
    type Value = Format;
    type Error = FormatParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let format = match string {
            "argb8888" => Format::ARGB8888,
            "xrgb8888" => Format::XRGB8888,
            "abgr8888" => Format::ABGR8888,
            "xbgr8888" => Format::XBGR8888,
            "r8" => Format::R8,
            "gr88" => Format::GR88,
            "rgb888" => Format::RGB888,
            "bgr888" => Format::BGR888,
            "rgba4444" => Format::RGBA4444,
            "rgbx4444" => Format::RGBX4444,
            "bgra4444" => Format::BGRA4444,
            "bgrx4444" => Format::BGRX4444,
            "rgb565" => Format::RGB565,
            "bgr565" => Format::BGR565,
            "rgba5551" => Format::RGBA5551,
            "rgbx5551" => Format::RGBX5551,
            "bgra5551" => Format::BGRA5551,
            "bgrx5551" => Format::BGRX5551,
            "argb1555" => Format::ARGB1555,
            "xrgb1555" => Format::XRGB1555,
            "argb2101010" => Format::ARGB2101010,
            "xrgb2101010" => Format::XRGB2101010,
            "abgr2101010" => Format::ABGR2101010,
            "xbgr2101010" => Format::XBGR2101010,
            "abgr16161616" => Format::ABGR16161616,
            "xbgr16161616" => Format::XBGR16161616,
            "abgr16161616f" => Format::ABGR16161616F,
            "xbgr16161616f" => Format::XBGR16161616F,
            _ => return Err(FormatParserError::UnknownFormat(string.to_string()).spanned(span)),
        };
        Ok(format)
    }
}
