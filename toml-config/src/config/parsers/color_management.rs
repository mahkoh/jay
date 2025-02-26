use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, bol, opt},
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
pub enum ColorManagementParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ColorManagementParser<'a>(pub &'a Context<'a>);

#[derive(Clone, Debug)]
pub struct ColorManagement {
    pub enabled: Option<bool>,
}

impl Parser for ColorManagementParser<'_> {
    type Value = ColorManagement;
    type Error = ColorManagementParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (enabled,) = ext.extract((opt(bol("enabled")),))?;
        Ok(ColorManagement {
            enabled: enabled.despan(),
        })
    }
}
