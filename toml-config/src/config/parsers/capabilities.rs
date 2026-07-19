use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use jay_config::client::CC_DATA_CONTROL;
use jay_config::client::CC_DRM_LEASE;
use jay_config::client::CC_FOREIGN_TOPLEVEL_LIST;
use jay_config::client::CC_FOREIGN_TOPLEVEL_MANAGER;
use jay_config::client::CC_GAMMA_CONTROL_MANAGER;
use jay_config::client::CC_HEAD_MANAGER;
use jay_config::client::CC_IDLE_NOTIFIER;
use jay_config::client::CC_INPUT_METHOD;
use jay_config::client::CC_LAYER_SHELL;
use jay_config::client::CC_SCREENCOPY;
use jay_config::client::CC_SEAT_MANAGER;
use jay_config::client::CC_SESSION_LOCK;
use jay_config::client::CC_VIRTUAL_KEYBOARD;
use jay_config::client::CC_VIRTUAL_POINTER;
use jay_config::client::CC_WORKSPACE_MANAGER;
use jay_config::client::ClientCapabilities;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CapabilitiesParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown capability `{}`", .0)]
    UnknownCapability(String),
}

pub struct CapabilitiesParser;

impl Parser for CapabilitiesParser {
    type Value = ClientCapabilities;
    type Error = CapabilitiesParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array, DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let ty = match string {
            "none" => ClientCapabilities(0),
            "all" => ClientCapabilities(!0),
            "data-control" => CC_DATA_CONTROL,
            "virtual-keyboard" => CC_VIRTUAL_KEYBOARD,
            "foreign-toplevel-list" => CC_FOREIGN_TOPLEVEL_LIST,
            "idle-notifier" => CC_IDLE_NOTIFIER,
            "session-lock" => CC_SESSION_LOCK,
            "layer-shell" => CC_LAYER_SHELL,
            "screencopy" => CC_SCREENCOPY,
            "seat-manager" => CC_SEAT_MANAGER,
            "drm-lease" => CC_DRM_LEASE,
            "input-method" => CC_INPUT_METHOD,
            "workspace-manager" => CC_WORKSPACE_MANAGER,
            "foreign-toplevel-manager" => CC_FOREIGN_TOPLEVEL_MANAGER,
            "head-manager" => CC_HEAD_MANAGER,
            "gamma-control-manager" => CC_GAMMA_CONTROL_MANAGER,
            "virtual-pointer" => CC_VIRTUAL_POINTER,
            _ => {
                return Err(
                    CapabilitiesParserError::UnknownCapability(string.to_owned()).spanned(span),
                );
            }
        };
        Ok(ty)
    }

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut ty = ClientCapabilities(0);
        for el in array {
            ty |= el.parse(&mut CapabilitiesParser)?;
        }
        Ok(ty)
    }
}
