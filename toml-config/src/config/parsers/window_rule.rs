use {
    crate::{
        config::{
            WindowMatch, WindowRule,
            context::Context,
            extractor::{Extractor, ExtractorError, bol, opt, recover, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::{ActionParser, ActionParserError},
                window_match::{WindowMatchParser, WindowMatchParserError},
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

pub struct WindowRuleParser<'a>(pub &'a Context<'a>);

impl Parser for WindowRuleParser<'_> {
    type Value = WindowRule;
    type Error = WindowRuleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (name, match_val, action_val, latch_val, auto_focus) = ext.extract((
            opt(str("name")),
            opt(val("match")),
            opt(val("action")),
            opt(val("latch")),
            recover(opt(bol("auto-focus"))),
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
        })
    }
}

pub struct WindowRulesParser<'a>(pub &'a Context<'a>);

impl Parser for WindowRulesParser<'_> {
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
