use {
    crate::toml::{
        toml_span::{Span, Spanned, SpannedExt},
        toml_value::Value,
    },
    indexmap::IndexMap,
    std::{
        error::Error,
        fmt::{self, Display, Formatter},
    },
};

#[derive(Copy, Clone, Debug)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Table,
}

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            DataType::String => "a string",
            DataType::Integer => "an integer",
            DataType::Float => "a float",
            DataType::Boolean => "a bool",
            DataType::Array => "an array",
            DataType::Table => "a table",
        };
        f.write_str(s)
    }
}

pub struct DataTypes(&'static [DataType]);

impl Display for DataTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let d = self.0;
        match d.len() {
            0 => Ok(()),
            1 => d[0].fmt(f),
            2 => write!(f, "{} or {}", d[0], d[1]),
            _ => {
                let mut first = true;
                #[allow(clippy::needless_range_loop)]
                for i in 0..d.len() - 1 {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    d[i].fmt(f)?;
                }
                write!(f, ", or {}", d[d.len() - 1])
            }
        }
    }
}

#[derive(Debug)]
pub struct UnexpectedDataType(&'static [DataType], DataType);

impl Display for UnexpectedDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Expected {} but found {}", DataTypes(self.0), self.1)
    }
}

impl Error for UnexpectedDataType {}

pub type ParseResult<P> = Result<<P as Parser>::Value, Spanned<<P as Parser>::Error>>;

pub trait Parser {
    type Value;
    type Error: From<UnexpectedDataType>;

    const EXPECTED: &'static [DataType];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let _ = string;
        expected(self, span, DataType::String)
    }

    fn parse_integer(&mut self, span: Span, integer: i64) -> ParseResult<Self> {
        let _ = integer;
        expected(self, span, DataType::Integer)
    }

    fn parse_float(&mut self, span: Span, float: f64) -> ParseResult<Self> {
        let _ = float;
        expected(self, span, DataType::Float)
    }

    fn parse_bool(&mut self, span: Span, bool: bool) -> ParseResult<Self> {
        let _ = bool;
        expected(self, span, DataType::Boolean)
    }

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let _ = array;
        expected(self, span, DataType::Array)
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let _ = table;
        expected(self, span, DataType::Table)
    }
}

fn expected<P: Parser + ?Sized>(_p: &P, span: Span, actual: DataType) -> ParseResult<P> {
    Err(P::Error::from(UnexpectedDataType(P::EXPECTED, actual)).spanned(span))
}
