use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::fltorint;
use crate::config::extractor::opt;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use std::time::Duration;
use std::time::TryFromFloatSecsError;
use thiserror::Error;

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

pub struct CleanLogsOlderThanParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for CleanLogsOlderThanParser<'_, '_> {
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
