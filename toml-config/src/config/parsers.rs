use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::toml_span::Span,
    },
    thiserror::Error,
};

pub mod action;
mod actions;
mod capabilities;
mod client_match;
mod client_rule;
mod color;
pub mod color_management;
pub mod config;
mod connector;
mod connector_match;
mod content_type;
mod drm_device;
mod drm_device_match;
mod env;
pub mod exec;
mod fallback_output_mode;
pub mod float;
pub mod focus_history;
mod format;
mod gfx_api;
mod idle;
mod input;
mod input_match;
pub mod input_mode;
pub mod keymap;
mod libei;
mod log_level;
pub mod mark_id;
mod mode;
pub mod modified_keysym;
mod output;
mod output_match;
mod repeat_rate;
pub mod shortcuts;
mod simple_im;
mod status;
mod tearing;
mod theme;
mod tile_state;
mod ui_drag;
mod vrr;
mod window_match;
mod window_rule;
mod window_type;
mod workspace_display_order;
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
