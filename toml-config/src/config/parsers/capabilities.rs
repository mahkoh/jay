use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    jay_config::client::{
        CC_DATA_CONTROL, CC_DRM_LEASE, CC_FOREIGN_TOPLEVEL_LIST, CC_FOREIGN_TOPLEVEL_MANAGER,
        CC_HEAD_MANAGER, CC_IDLE_NOTIFIER, CC_INPUT_METHOD, CC_LAYER_SHELL, CC_SCREENCOPY,
        CC_SEAT_MANAGER, CC_SESSION_LOCK, CC_VIRTUAL_KEYBOARD, CC_WORKSPACE_MANAGER,
        ClientCapabilities,
    },
    thiserror::Error,
};

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
