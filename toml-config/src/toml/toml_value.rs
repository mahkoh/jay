use {
    crate::toml::toml_span::Spanned,
    indexmap::IndexMap,
    std::{
        cmp::Ordering,
        fmt::{Debug, Formatter},
    },
};

pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<Spanned<Value>>),
    Table(IndexMap<Spanned<String>, Spanned<Value>>),
}

impl Value {
    pub fn name(&self) -> &'static str {
        match self {
            Value::String(_) => "a string",
            Value::Integer(_) => "an integer",
            Value::Float(_) => "a float",
            Value::Boolean(_) => "a boolean",
            Value::Array(_) => "an array",
            Value::Table(_) => "a table",
        }
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(v) => v.fmt(f),
            Value::Integer(v) => v.fmt(f),
            Value::Float(v) => v.fmt(f),
            Value::Boolean(v) => v.fmt(f),
            Value::Array(v) => v.fmt(f),
            Value::Table(v) => v.fmt(f),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(v1), Value::String(v2)) => v1 == v2,
            (Value::Integer(v1), Value::Integer(v2)) => v1 == v2,
            (Value::Float(v1), Value::Float(v2)) => {
                if v1.is_nan() && v2.is_nan() {
                    true
                } else {
                    v1.total_cmp(v2) == Ordering::Equal
                }
            }
            (Value::Boolean(v1), Value::Boolean(v2)) => v1 == v2,
            (Value::Array(v1), Value::Array(v2)) => v1 == v2,
            (Value::Table(v1), Value::Table(v2)) => v1 == v2,
            _ => false,
        }
    }
}
