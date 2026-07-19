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
pub enum FloatParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct FloatParser<'a, 'b>(pub &'a Context<'b>);

#[derive(Debug, Clone)]
pub struct Float {
    pub show_pin_icon: Option<bool>,
}

impl Parser for FloatParser<'_, '_> {
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
