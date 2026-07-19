use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::StringParser;
use crate::config::parsers::StringParserError;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

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
