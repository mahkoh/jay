use {
    crate::{
        config::{
            OutputMatch,
            context::Context,
            extractor::{Extractor, ExtractorError, opt, str},
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
pub enum OutputMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct OutputMatchParser<'a>(pub &'a Context<'a>);

impl Parser for OutputMatchParser<'_> {
    type Value = OutputMatch;
    type Error = OutputMatchParserError;
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
        Ok(OutputMatch::Any(res))
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (name, connector, serial_number, manufacturer, model) = ext.extract((
            opt(str("name")),
            opt(str("connector")),
            opt(str("serial-number")),
            opt(str("manufacturer")),
            opt(str("model")),
        ))?;
        if let Some(name) = name {
            self.0.used.borrow_mut().outputs.push(name.into());
        }
        Ok(OutputMatch::All {
            name: name.despan_into(),
            connector: connector.despan_into(),
            serial_number: serial_number.despan_into(),
            manufacturer: manufacturer.despan_into(),
            model: model.despan_into(),
        })
    }
}
