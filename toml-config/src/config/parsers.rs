use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::Span,
    },
    thiserror::Error,
};

pub mod action;
mod actions;
mod client_match;
mod client_rule;
mod color;
pub mod color_management;
pub mod config;
mod connector;
mod connector_match;
mod drm_device;
mod drm_device_match;
mod env;
pub mod exec;
pub mod float;
mod format;
mod gfx_api;
mod idle;
mod input;
mod input_match;
pub mod keymap;
mod libei;
mod log_level;
mod mode;
pub mod modified_keysym;
mod output;
mod output_match;
mod repeat_rate;
pub mod shortcuts;
mod status;
mod tearing;
mod theme;
mod tile_state;
mod ui_drag;
mod vrr;
mod window_match;
mod window_rule;
mod window_type;
mod xwayland;

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
