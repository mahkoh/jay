use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, fltorint, opt},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    std::time::{Duration, TryFromFloatSecsError},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum CleanLogsOlderThanParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("At least one of the `weeks` or `days` fields must be specified")]
    WeeksOrDays,
    #[error("Duration is invalid")]
    InvalidDuration(#[source] TryFromFloatSecsError),
}

pub struct CleanLogsOlderThanParser<'a>(pub &'a Context<'a>);

impl Parser for CleanLogsOlderThanParser<'_> {
    type Value = Duration;
    type Error = CleanLogsOlderThanParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (weeks, days) = ext.extract((opt(fltorint("weeks")), opt(fltorint("days"))))?;
        if weeks.is_none() && days.is_none() {
            return Err(CleanLogsOlderThanParserError::WeeksOrDays.spanned(span));
        }
        const SECONDS_IN_WEEK: f64 = 604800.0;
        const SECONDS_IN_DAY: f64 = 86400.0;
        let duration = Duration::try_from_secs_f64(
            weeks.despan().unwrap_or_default() * SECONDS_IN_WEEK
                + days.despan().unwrap_or_default() * SECONDS_IN_DAY,
        )
        .map_err(CleanLogsOlderThanParserError::InvalidDuration)
        .map_err(|e| e.spanned(span))?;
        Ok(duration)
    }
}
