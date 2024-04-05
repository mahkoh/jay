use {
    crate::{
        config::{
            context::Context,
            extractor::{s32, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            RepeatRate,
        },
        toml::{
            toml_span::{Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum RepeatRateParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct RepeatRateParser<'a>(pub &'a Context<'a>);

impl Parser for RepeatRateParser<'_> {
    type Value = RepeatRate;
    type Error = RepeatRateParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (rate, delay) = ext.extract((s32("rate"), s32("delay")))?;
        Ok(RepeatRate {
            rate: rate.value,
            delay: delay.value,
        })
    }
}
