use {
    crate::{
        config::{
            ConfigConnector,
            context::Context,
            extractor::{Extractor, ExtractorError, bol, opt, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::connector_match::{ConnectorMatchParser, ConnectorMatchParserError},
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
pub enum ConnectorParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] ConnectorMatchParserError),
}

pub struct ConnectorParser<'a>(pub &'a Context<'a>);

impl Parser for ConnectorParser<'_> {
    type Value = ConfigConnector;
    type Error = ConnectorParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (match_val, enabled) = ext.extract((val("match"), opt(bol("enabled"))))?;
        Ok(ConfigConnector {
            match_: match_val.parse_map(&mut ConnectorMatchParser(self.0))?,
            enabled: enabled.despan().unwrap_or(true),
        })
    }
}

pub struct ConnectorsParser<'a>(pub &'a Context<'a>);

impl Parser for ConnectorsParser<'_> {
    type Value = Vec<ConfigConnector>;
    type Error = ConnectorParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut ConnectorParser(self.0)) {
                Ok(o) => res.push(o),
                Err(e) => {
                    log::warn!("Could not parse connector: {}", self.0.error(e));
                }
            }
        }
        Ok(res)
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        log::warn!(
            "`connectors` value should be an array: {}",
            self.0.error3(span)
        );
        ConnectorParser(self.0)
            .parse_table(span, table)
            .map(|v| vec![v])
    }
}
