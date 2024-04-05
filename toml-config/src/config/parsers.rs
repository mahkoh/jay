use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::Span,
    },
    thiserror::Error,
};

pub mod action;
mod color;
pub mod config;
mod connector;
mod connector_match;
mod drm_device;
mod drm_device_match;
mod env;
pub mod exec;
mod gfx_api;
mod idle;
mod input;
mod input_match;
pub mod keymap;
mod log_level;
mod mode;
pub mod modified_keysym;
mod output;
mod output_match;
mod repeat_rate;
pub mod shortcuts;
mod status;
mod theme;

#[derive(Debug, Error)]
pub enum StringParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
}

pub struct StringParser;

impl Parser for StringParser {
    type Value = String;
    type Error = StringParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, _span: Span, string: &str) -> ParseResult<Self> {
        Ok(string.to_string())
    }
}
