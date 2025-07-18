use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, bol, opt, recover},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
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
pub enum FocusHistoryParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FocusHistoryParser<'a>(pub &'a Context<'a>);

#[derive(Debug, Clone)]
pub struct FocusHistory {
    pub only_visible: Option<bool>,
    pub same_workspace: Option<bool>,
}

impl Parser for FocusHistoryParser<'_> {
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
