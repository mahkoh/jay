use {
    crate::{
        config::{
            ClientMatch, GenericMatch, MatchExactly,
            context::Context,
            extractor::{Extractor, ExtractorError, arr, n32, opt, str, val},
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
pub enum ClientMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ClientMatchParser<'a>(pub &'a Context<'a>);

impl Parser for ClientMatchParser<'_> {
    type Value = ClientMatch;
    type Error = ClientMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let ((name, not_val, all_val, any_val, exactly_val),) = ext.extract(((
            opt(str("name")),
            opt(val("not")),
            opt(arr("all")),
            opt(arr("any")),
            opt(val("exactly")),
        ),))?;
        let mut not = None;
        if let Some(value) = not_val {
            not = Some(Box::new(value.parse(&mut ClientMatchParser(self.0))?));
        }
        macro_rules! list {
            ($val:expr) => {{
                let mut list = None;
                if let Some(value) = $val {
                    let mut res = vec![];
                    for value in value.value {
                        res.push(value.parse(&mut ClientMatchParser(self.0))?);
                    }
                    list = Some(res);
                }
                list
            }};
        }
        let all = list!(all_val);
        let any = list!(any_val);
        let mut exactly = None;
        if let Some(value) = exactly_val {
            exactly = Some(value.parse(&mut ClientMatchExactlyParser(self.0))?);
        }
        Ok(ClientMatch {
            generic: GenericMatch {
                name: name.despan_into(),
                not,
                all,
                any,
                exactly,
            },
        })
    }
}

pub struct ClientMatchExactlyParser<'a>(pub &'a Context<'a>);

impl Parser for ClientMatchExactlyParser<'_> {
    type Value = MatchExactly<ClientMatch>;
    type Error = ClientMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (num, list_val) = ext.extract((n32("num"), arr("list")))?;
        let mut list = vec![];
        for el in list_val.value {
            list.push(el.parse(&mut ClientMatchParser(self.0))?);
        }
        Ok(MatchExactly {
            num: num.value as _,
            list,
        })
    }
}
