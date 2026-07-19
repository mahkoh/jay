use crate::config::Status;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::exec::ExecParser;
use crate::config::parsers::exec::ExecParserError;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::status::MessageFormat;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatusParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Exec(#[from] ExecParserError),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Expected `plain`, `pango`, or `i3bar` but found {0}")]
    UnknownFormat(String),
}

pub struct StatusParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for StatusParser<'_, '_> {
    type Value = Status;
    type Error = StatusParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (format, exec, separator) = ext.extract((
            opt(str("format")),
            val("exec"),
            recover(opt(str("i3bar-separator"))),
        ))?;
        let format = match format {
            Some(f) => match f.value {
                "plain" => MessageFormat::Plain,
                "pango" => MessageFormat::Pango,
                "i3bar" => MessageFormat::I3Bar,
                _ => {
                    return Err(
                        StatusParserError::UnknownFormat(f.value.to_string()).spanned(f.span)
                    );
                }
            },
            _ => MessageFormat::Plain,
        };
        let exec = exec.parse_map(&mut ExecParser(self.0))?;
        let separator = match separator {
            None => None,
            Some(sep) if format == MessageFormat::I3Bar => Some(sep.value.to_string()),
            Some(sep) => {
                log::warn!(
                    "Separator has no effect for format {format:?}: {}",
                    self.0.error3(sep.span)
                );
                None
            }
        };
        Ok(Status {
            format,
            exec,
            separator,
        })
    }
}
