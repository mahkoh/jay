use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::str;
use crate::config::keycodes::keycode_from_name;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarkIdParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("MarkId must have exactly one field set")]
    ExactlyOneField,
    #[error("Unknown key {0}")]
    UnknownKey(String),
}

pub struct MarkIdParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for MarkIdParser<'_, '_> {
    type Value = u32;
    type Error = MarkIdParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (key, name) = ext.extract((opt(str("key")), opt(str("name"))))?;
        let id = match (key, name) {
            (None, None) | (Some(_), Some(_)) => {
                return Err(MarkIdParserError::ExactlyOneField.spanned(span));
            }
            (Some(key), _) => match keycode_from_name(key.value) {
                Some(c) => c,
                _ => return Err(key.map(|s| MarkIdParserError::UnknownKey(s.to_string()))),
            },
            (_, Some(name)) => {
                let mn = &mut *self.0.mark_names.borrow_mut();
                let len = mn.len() as u32;
                *mn.entry(name.value.to_string())
                    .or_insert(u32::MAX - 8 - len)
            }
        };
        Ok(id)
    }
}
