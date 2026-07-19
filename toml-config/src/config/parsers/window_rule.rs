use crate::config::WindowMatch;
use crate::config::WindowRule;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::action::ActionParser;
use crate::config::parsers::action::ActionParserError;
use crate::config::parsers::tile_state::TileStateParser;
use crate::config::parsers::window_match::WindowMatchParser;
use crate::config::parsers::window_match::WindowMatchParserError;
use crate::config::spanned::SpannedErrorExt;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowRuleParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] WindowMatchParserError),
    #[error(transparent)]
    Action(ActionParserError),
    #[error(transparent)]
    Latch(ActionParserError),
}

pub struct WindowRuleParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for WindowRuleParser<'_, '_> {
    type Value = WindowRule;
    type Error = WindowRuleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (name, match_val, action_val, latch_val, auto_focus, initial_tile_state_val) = ext
            .extract((
                opt(str("name")),
                opt(val("match")),
                opt(val("action")),
                opt(val("latch")),
                recover(opt(bol("auto-focus"))),
                opt(val("initial-tile-state")),
            ))?;
        let mut action = None;
        if let Some(value) = action_val {
            action = Some(
                value
                    .parse(&mut ActionParser(self.0))
                    .map_spanned_err(WindowRuleParserError::Action)?,
            );
        }
        let mut latch = None;
        if let Some(value) = latch_val {
            latch = Some(
                value
                    .parse(&mut ActionParser(self.0))
                    .map_spanned_err(WindowRuleParserError::Latch)?,
            );
        }
        let mut initial_tile_state = None;
        if let Some(value) = initial_tile_state_val {
            match value.parse(&mut TileStateParser) {
                Ok(v) => initial_tile_state = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse the initial tile state: {}",
                        self.0.error(e)
                    );
                }
            }
        }
        let match_ = match match_val {
            None => WindowMatch::default(),
            Some(m) => m.parse_map(&mut WindowMatchParser(self.0))?,
        };
        Ok(WindowRule {
            name: name.despan_into(),
            match_,
            action,
            latch,
            auto_focus: auto_focus.despan(),
            initial_tile_state,
        })
    }
}

pub struct WindowRulesParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for WindowRulesParser<'_, '_> {
    type Value = Vec<WindowRule>;
    type Error = WindowRuleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut WindowRuleParser(self.0)) {
                Ok(o) => res.push(o),
                Err(e) => {
                    log::warn!("Could not parse window rule: {}", self.0.error(e));
                }
            }
        }
        Ok(res)
    }
}
