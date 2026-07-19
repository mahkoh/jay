use crate::config::UiDrag;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::int;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::exec::ExecParserError;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiDragParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Exec(#[from] ExecParserError),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct UiDragParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for UiDragParser<'_, '_> {
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
