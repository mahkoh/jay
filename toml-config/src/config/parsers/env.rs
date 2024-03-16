use {
    crate::{
        config::{
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{StringParser, StringParserError},
        },
        toml::{
            toml_span::{Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum EnvParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    String(#[from] StringParserError),
}

pub struct EnvParser;

impl Parser for EnvParser {
    type Value = Vec<(String, String)>;
    type Error = EnvParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut envs = vec![];
        for (k, v) in table {
            envs.push((k.value.to_string(), v.parse_map(&mut StringParser)?));
        }
        Ok(envs)
    }
}
