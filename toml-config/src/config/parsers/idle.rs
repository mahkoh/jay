use {
    crate::{
        config::{
            context::Context,
            extractor::{n64, opt, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    std::time::Duration,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum IdleParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct IdleParser<'a>(pub &'a Context<'a>);

impl Parser for IdleParser<'_> {
    type Value = Duration;
    type Error = IdleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (minutes, seconds) = ext.extract((opt(n64("minutes")), opt(n64("seconds"))))?;
        let idle = Duration::from_secs(
            minutes.despan().unwrap_or_default() * 60 + seconds.despan().unwrap_or_default(),
        );
        Ok(idle)
    }
}
