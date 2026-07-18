use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, s32},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
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
pub enum FloatingSizeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FloatingSizeParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for FloatingSizeParser<'_, '_> {
    type Value = (i32, i32);
    type Error = FloatingSizeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (width, height) = ext.extract((s32("width"), s32("height")))?;
        Ok((width.value, height.value))
    }
}

#[derive(Debug, Error)]
pub enum FloatingPositionParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FloatingPositionParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for FloatingPositionParser<'_, '_> {
    type Value = (i32, i32);
    type Error = FloatingPositionParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (x, y) = ext.extract((s32("x"), s32("y")))?;
        Ok((x.value, y.value))
    }
}
