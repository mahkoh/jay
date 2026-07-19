use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::n64;
use crate::config::extractor::opt;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionsParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct TransactionsParser<'a, 'b>(pub &'a Context<'b>);

#[derive(Clone, Debug)]
pub struct Transactions {
    pub transaction_timeout: Option<Duration>,
    pub configure_timeout: Option<Duration>,
}

impl Parser for TransactionsParser<'_, '_> {
    type Value = Transactions;
    type Error = TransactionsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        const TRANSACTION_TIMEOUT: &str = "transaction-timeout";
        const CONFIGURE_TIMEOUT: &str = "configure-timeout";
        let mut ext = Extractor::new(self.0, span, table);
        let (timeout_val, transaction_timeout_val, configure_timeout_val) = ext.extract((
            opt(val("timeout")),
            opt(val(TRANSACTION_TIMEOUT)),
            opt(val(CONFIGURE_TIMEOUT)),
        ))?;
        let mut configure_timeout = None;
        let mut transaction_timeout = None;
        if let Some(v) = timeout_val {
            match v.parse(&mut TimeoutParser(self.0)) {
                Ok(v) => {
                    configure_timeout = Some(v);
                    transaction_timeout = Some(v);
                }
                Err(e) => {
                    log::error!("Could not parse timeout field: {}", self.0.error(e));
                }
            }
        }
        for (name, field, val) in [
            (
                TRANSACTION_TIMEOUT,
                &mut transaction_timeout,
                transaction_timeout_val,
            ),
            (
                CONFIGURE_TIMEOUT,
                &mut configure_timeout,
                configure_timeout_val,
            ),
        ] {
            if let Some(v) = val {
                match v.parse(&mut TimeoutParser(self.0)) {
                    Ok(v) => {
                        *field = Some(v);
                    }
                    Err(e) => {
                        log::error!("Could not parse {name} field: {}", self.0.error(e));
                    }
                }
            }
        }
        Ok(Transactions {
            transaction_timeout,
            configure_timeout,
        })
    }
}

struct TimeoutParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for TimeoutParser<'_, '_> {
    type Value = Duration;
    type Error = TransactionsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (millis, micros) = ext.extract((opt(n64("millis")), opt(n64("micros"))))?;
        let millis = millis.despan().unwrap_or_default();
        let micros = micros.despan().unwrap_or_default();
        Ok(Duration::from_micros(
            millis.saturating_mul(1_000).saturating_add(micros),
        ))
    }
}
