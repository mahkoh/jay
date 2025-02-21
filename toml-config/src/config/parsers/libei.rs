use {
    crate::{
        config::{
            Libei,
            context::Context,
            extractor::{Extractor, ExtractorError, bol, opt, recover},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum LibeiParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct LibeiParser<'a>(pub &'a Context<'a>);

impl Parser for LibeiParser<'_> {
    type Value = Libei;
    type Error = LibeiParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let enable_socket = ext.extract(recover(opt(bol("enable-socket"))))?;
        Ok(Libei {
            enable_socket: enable_socket.despan(),
        })
    }
}
