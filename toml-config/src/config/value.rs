use crate::{
    config::parser::{ParseResult, Parser},
    toml::{toml_span::Span, toml_value::Value},
};

impl Value {
    pub fn parse<P: Parser>(&self, span: Span, parser: &mut P) -> ParseResult<P> {
        match self {
            Value::String(a) => parser.parse_string(span, a),
            Value::Integer(a) => parser.parse_integer(span, *a),
            Value::Float(a) => parser.parse_float(span, *a),
            Value::Boolean(a) => parser.parse_bool(span, *a),
            Value::Array(a) => parser.parse_array(span, a),
            Value::Table(a) => parser.parse_table(span, a),
        }
    }
}
