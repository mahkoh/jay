use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, n64, opt, val},
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

pub struct Idle {
    pub timeout: Option<Duration>,
    pub grace_period: Option<Duration>,
}

impl Parser for IdleParser<'_> {
    type Value = Idle;
    type Error = IdleParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (minutes, seconds, grace_period_val) = ext.extract((
            opt(n64("minutes")),
            opt(n64("seconds")),
            opt(val("grace-period")),
        ))?;
        let mut timeout = None;
        if minutes.is_some() || seconds.is_some() {
            timeout = Some(parse_duration(&minutes, &seconds));
        }
        let mut grace_period = None;
        if let Some(gp) = grace_period_val {
            grace_period = Some(gp.parse(&mut GracePeriodParser(self.0))?);
        }
        Ok(Idle {
            timeout,
            grace_period,
        })
    }
}

struct GracePeriodParser<'a>(pub &'a Context<'a>);

impl Parser for GracePeriodParser<'_> {
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
        let grace_period = parse_duration(&minutes, &seconds);
        Ok(grace_period)
    }
}

fn parse_duration(minutes: &Option<Spanned<u64>>, seconds: &Option<Spanned<u64>>) -> Duration {
    Duration::from_secs(
        minutes.despan().unwrap_or_default() * 60 + seconds.despan().unwrap_or_default(),
    )
}
