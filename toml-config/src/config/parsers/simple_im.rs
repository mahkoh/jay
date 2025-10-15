use {
    crate::{
        config::{
            SimpleIm,
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
pub enum SimpleImParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct SimpleImParser<'a>(pub &'a Context<'a>);

impl Parser for SimpleImParser<'_> {
    type Value = SimpleIm;
    type Error = SimpleImParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (enabled,) = ext.extract((recover(opt(bol("enabled"))),))?;
        Ok(SimpleIm {
            enabled: enabled.despan(),
        })
    }
}
