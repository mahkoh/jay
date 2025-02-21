use {
    crate::{
        config::{
            UiDrag,
            context::Context,
            extractor::{Extractor, ExtractorError, bol, int, opt, recover},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::exec::ExecParserError,
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
pub enum UiDragParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Exec(#[from] ExecParserError),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct UiDragParser<'a>(pub &'a Context<'a>);

impl Parser for UiDragParser<'_> {
    type Value = UiDrag;
    type Error = UiDragParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (enabled, threshold) =
            ext.extract((recover(opt(bol("enabled"))), recover(opt(int("threshold")))))?;
        Ok(UiDrag {
            enabled: enabled.despan(),
            threshold: threshold.despan().map(|v| v as i32),
        })
    }
}
