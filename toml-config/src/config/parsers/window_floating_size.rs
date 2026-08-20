use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::s32;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowFloatingSizeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("The `width` and `height` fields must be positive")]
    NotPositive,
}

pub struct WindowFloatingSizeParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

impl Parser for WindowFloatingSizeParser<'_, '_, '_> {
    type Value = (i32, i32);
    type Error = WindowFloatingSizeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            width, //
            height,
        ) = ext.extract((
            s32("width"), //
            s32("height"),
        ))?;
        if width.value <= 0 || height.value <= 0 {
            return Err(WindowFloatingSizeParserError::NotPositive.spanned(span));
        }
        Ok((width.value, height.value))
    }
}
