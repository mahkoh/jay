use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FocusHistoryParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FocusHistoryParser<'a, 'b>(pub &'a Context<'b>);

#[derive(Debug, Clone)]
pub struct FocusHistory {
    pub only_visible: Option<bool>,
    pub same_workspace: Option<bool>,
}

impl Parser for FocusHistoryParser<'_, '_> {
    type Value = FocusHistory;
    type Error = FocusHistoryParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (only_visible, same_workspace) = ext.extract((
            recover(opt(bol("only-visible"))),
            recover(opt(bol("same-workspace"))),
        ))?;
        Ok(FocusHistory {
            only_visible: only_visible.despan(),
            same_workspace: same_workspace.despan(),
        })
    }
}
