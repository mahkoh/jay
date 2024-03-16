use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, str, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            ConnectorMatch,
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
pub enum ConnectorMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ConnectorMatchParser<'a>(pub &'a Context<'a>);

impl<'a> Parser for ConnectorMatchParser<'a> {
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
