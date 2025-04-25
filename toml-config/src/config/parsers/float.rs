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
pub enum FloatParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FloatParser<'a>(pub &'a Context<'a>);

#[derive(Debug, Clone)]
pub struct Float {
    pub show_pin_icon: Option<bool>,
}

impl Parser for FloatParser<'_> {
    type Value = Float;
    type Error = FloatParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (show_pin_icon,) = ext.extract((recover(opt(bol("show-pin-icon"))),))?;
        Ok(Float {
            show_pin_icon: show_pin_icon.despan(),
        })
    }
}
