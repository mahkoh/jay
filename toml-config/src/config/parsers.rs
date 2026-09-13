use crate::config::context::Context;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::spanned::SpannedErrorExt;
use crate::toml::toml_parser;
use crate::toml::toml_parser::ParserError;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use std::cell::RefCell;
use std::error::Error;
use thiserror::Error;

pub mod action;
mod actions;
mod capabilities;
mod clean_logs_older_than;
pub mod client_match;
mod client_rule;
mod color;
pub mod color_management;
pub mod config;
mod connector;
mod connector_match;
mod content_type;
mod drm_device;
mod drm_device_match;
mod egui;
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
mod mouse_follows_focus_mode;
mod output;
mod output_match;
mod repeat_rate;
pub mod session_management;
pub mod shortcuts;
mod simple_im;
mod status;
mod tearing;
mod theme;
mod tile_state;
pub mod transactions;
pub mod trigger;
mod ui_drag;
mod vrr;
mod warp_target;
mod window_floating_position;
mod window_floating_size;
pub mod window_match;
mod window_rule;
mod window_type;
pub mod workspace;
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

fn parse_object<'a, V, E>(
    input: &'a [u8],
    name: &'static str,
    parse: impl FnOnce(&Spanned<Value>, &Context<'a, '_>) -> Result<V, Spanned<E>>,
) -> Result<V, Box<dyn Error + 'a>>
where
    E: Error + 'static,
{
    let mut workspaces = Default::default();
    let cx = Context::<'a, '_> {
        input,
        used: Default::default(),
        mark_names: &Default::default(),
        workspaces: RefCell::new(&mut workspaces),
        counters: Default::default(),
    };
    #[derive(Debug, Error)]
    #[error("Could not parse the toml")]
    struct Toml(#[source] ParserError);
    #[derive(Debug, Error)]
    #[error("Could not parse the {1}")]
    struct Value<T: Error>(#[source] T, &'static str);
    let value = toml_parser::parse(input, &cx)
        .map_spanned_err(Toml)
        .map_err(|e| cx.error(e))?;
    let value = parse(&value, &cx)
        .map_spanned_err(|e| Value(e, name))
        .map_err(|e| cx.error(e))?;
    Ok(value)
}
