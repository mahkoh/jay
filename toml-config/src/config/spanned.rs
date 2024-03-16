use crate::{
    config::parser::{ParseResult, Parser},
    toml::{toml_span::Spanned, toml_value::Value},
};

impl Spanned<&Value> {
    pub fn parse<P: Parser>(&self, parser: &mut P) -> ParseResult<P> {
        self.value.parse(self.span, parser)
    }

    pub fn parse_map<P: Parser, E>(
        &self,
        parser: &mut P,
    ) -> Result<<P as Parser>::Value, Spanned<E>>
    where
        <P as Parser>::Error: Into<E>,
    {
        self.parse(parser).map_spanned_err(|e| e.into())
    }
}

impl Spanned<Value> {
    pub fn parse<P: Parser>(&self, parser: &mut P) -> ParseResult<P> {
        self.as_ref().parse(parser)
    }

    pub fn parse_map<P: Parser, E>(
        &self,
        parser: &mut P,
    ) -> Result<<P as Parser>::Value, Spanned<E>>
    where
        <P as Parser>::Error: Into<E>,
    {
        self.as_ref().parse_map(parser)
    }
}

pub trait SpannedErrorExt {
    type T;
    type E;

    fn map_spanned_err<U, F>(self, f: F) -> Result<Self::T, Spanned<U>>
    where
        F: FnOnce(Self::E) -> U;
}

impl<T, E> SpannedErrorExt for Result<T, Spanned<E>> {
    type T = T;
    type E = E;

    fn map_spanned_err<U, F>(self, f: F) -> Result<Self::T, Spanned<U>>
    where
        F: FnOnce(Self::E) -> U,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(Spanned {
                span: e.span,
                value: f(e.value),
            }),
        }
    }
}
