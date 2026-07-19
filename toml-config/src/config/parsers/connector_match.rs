use crate::config::ConnectorMatch;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::str;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectorMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ConnectorMatchParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for ConnectorMatchParser<'_, '_> {
    type Value = ConnectorMatch;
    type Error = ConnectorMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(self) {
                Ok(m) => res.push(m),
                Err(e) => {
                    log::error!("Could not parse match rule: {}", self.0.error(e));
                }
            }
        }
        Ok(ConnectorMatch::Any(res))
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (connector,) = ext.extract((opt(str("name")),))?;
        Ok(ConnectorMatch::All {
            connector: connector.map(|v| v.value.to_owned()),
        })
    }
}
