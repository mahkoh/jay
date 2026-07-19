use crate::config::Mode;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::fltorint;
use crate::config::extractor::opt;
use crate::config::extractor::s32;
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
pub enum ModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ModeParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for ModeParser<'_, '_> {
    type Value = Mode;
    type Error = ModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (width, height, refresh_rate) =
            ext.extract((s32("width"), s32("height"), opt(fltorint("refresh-rate"))))?;
        Ok(Mode {
            width: width.value,
            height: height.value,
            refresh_rate: refresh_rate.despan(),
        })
    }
}
