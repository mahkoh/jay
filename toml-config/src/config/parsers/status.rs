use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, recover, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::exec::{ExecParser, ExecParserError},
            Status,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::status::MessageFormat,
    thiserror::Error,
};

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

pub struct StatusParser<'a>(pub &'a Context<'a>);

impl Parser for StatusParser<'_> {
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
                    )
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
