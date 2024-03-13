use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::video::GfxApi,
    thiserror::Error,
};

pub struct GfxApiParser;

#[derive(Debug, Error)]
pub enum GfxApiParserError {
    #[error(transparent)]
    DataType(#[from] UnexpectedDataType),
    #[error("Unknown API {0}")]
    Unknown(String),
}

impl Parser for GfxApiParser {
    type Value = GfxApi;
    type Error = GfxApiParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        use GfxApi::*;
        let api = match string.to_ascii_lowercase().as_str() {
            "opengl" => OpenGl,
            "vulkan" => Vulkan,
            _ => return Err(GfxApiParserError::Unknown(string.to_string()).spanned(span)),
        };
        Ok(api)
    }
}
