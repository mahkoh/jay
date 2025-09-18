use {
    crate::{
        config::{
            ClientMatch, ClientRule,
            context::Context,
            extractor::{Extractor, ExtractorError, opt, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::{ActionParser, ActionParserError},
                capabilities::CapabilitiesParser,
                client_match::{ClientMatchParser, ClientMatchParserError},
            },
            spanned::SpannedErrorExt,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ClientRuleParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] ClientMatchParserError),
    #[error(transparent)]
    Action(ActionParserError),
    #[error(transparent)]
    Latch(ActionParserError),
}

pub struct ClientRuleParser<'a>(pub &'a Context<'a>);

impl Parser for ClientRuleParser<'_> {
    type Value = ClientRule;
    type Error = ClientRuleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (name, match_val, action_val, latch_val, capabilities_val, bounding_capabilities_val) =
            ext.extract((
                opt(str("name")),
                opt(val("match")),
                opt(val("action")),
                opt(val("latch")),
                opt(val("capabilities")),
                opt(val("sandbox-bounding-capabilities")),
            ))?;
        let mut action = None;
        if let Some(value) = action_val {
            action = Some(
                value
                    .parse(&mut ActionParser(self.0))
                    .map_spanned_err(ClientRuleParserError::Action)?,
            );
        }
        let mut latch = None;
        if let Some(value) = latch_val {
            latch = Some(
                value
                    .parse(&mut ActionParser(self.0))
                    .map_spanned_err(ClientRuleParserError::Latch)?,
            );
        }
        let match_ = match match_val {
            None => ClientMatch::default(),
            Some(m) => m.parse_map(&mut ClientMatchParser(self.0))?,
        };
        let mut capabilities = None;
        if let Some(value) = capabilities_val {
            match value.parse(&mut CapabilitiesParser) {
                Ok(v) => capabilities = Some(v),
                Err(e) => {
                    log::warn!("Could not parse the capabilities: {}", self.0.error(e));
                }
            }
        }
        let mut bounding_capabilities = None;
        if let Some(value) = bounding_capabilities_val {
            match value.parse(&mut CapabilitiesParser) {
                Ok(v) => bounding_capabilities = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the bounding capabilities: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        Ok(ClientRule {
            name: name.despan_into(),
            match_,
            action,
            latch,
            capabilities,
            bounding_capabilities,
        })
    }
}

pub struct ClientRulesParser<'a>(pub &'a Context<'a>);

impl Parser for ClientRulesParser<'_> {
    type Value = Vec<ClientRule>;
    type Error = ClientRuleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut ClientRuleParser(self.0)) {
                Ok(o) => res.push(o),
                Err(e) => {
                    log::warn!("Could not parse client rule: {}", self.0.error(e));
                }
            }
        }
        Ok(res)
    }
}
