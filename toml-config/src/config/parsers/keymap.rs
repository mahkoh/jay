use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, str, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            ConfigKeymap,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::{
        config_dir,
        keyboard::{parse_keymap, Keymap},
    },
    std::{io, path::PathBuf},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum KeymapParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
    #[error("The keymap is invalid")]
    Invalid,
    #[error("Keymap table must contain at least one of `name`, `map`")]
    MissingField,
    #[error("Keymap must have both `name` and `map` fields in this context")]
    DefinitionRequired,
    #[error("Could not read {0}")]
    ReadFile(String, #[source] io::Error),
}

pub struct KeymapParser<'a> {
    pub cx: &'a Context<'a>,
    pub definition: bool,
}

impl Parser for KeymapParser<'_> {
    type Value = ConfigKeymap;
    type Error = KeymapParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Table];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        Ok(ConfigKeymap::Literal(parse(span, string)?))
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (mut name_val, mut map_val, mut path) =
            ext.extract((opt(str("name")), opt(str("map")), opt(str("path"))))?;
        if map_val.is_some() && path.is_some() {
            log::warn!(
                "Both `name` and `path` are specified. Ignoring `path`: {}",
                self.cx.error3(span)
            );
            path = None;
        }
        let file_content;
        if let Some(path) = path {
            let mut root = PathBuf::from(config_dir());
            root.push(path.value);
            file_content = match std::fs::read_to_string(&root) {
                Ok(c) => c,
                Err(e) => {
                    return Err(KeymapParserError::ReadFile(root.display().to_string(), e)
                        .spanned(path.span))
                }
            };
            map_val = Some(file_content.as_str().spanned(path.span));
        }
        if self.definition && (name_val.is_none() || map_val.is_none()) {
            return Err(KeymapParserError::DefinitionRequired.spanned(span));
        }
        if !self.definition && map_val.is_some() {
            if let Some(val) = name_val {
                log::warn!(
                    "Cannot use both `name` and `map` in this position. Ignoring `name`: {}",
                    self.cx.error3(val.span)
                );
            }
            name_val = None;
        }
        if let Some(name) = name_val {
            if self.definition {
                self.cx
                    .used
                    .borrow_mut()
                    .defined_keymaps
                    .insert(name.into());
            } else {
                self.cx.used.borrow_mut().keymaps.push(name.into());
            }
        }
        let res = match (name_val, map_val) {
            (Some(name_val), Some(map_val)) => ConfigKeymap::Defined {
                name: name_val.value.to_string(),
                map: parse(map_val.span, map_val.value)?,
            },
            (Some(name_val), None) => ConfigKeymap::Named(name_val.value.to_string()),
            (None, Some(map_val)) => ConfigKeymap::Literal(parse(map_val.span, map_val.value)?),
            (None, None) => return Err(KeymapParserError::MissingField.spanned(span)),
        };
        Ok(res)
    }
}

fn parse(span: Span, string: &str) -> Result<Keymap, Spanned<KeymapParserError>> {
    let map = parse_keymap(string);
    match map.is_valid() {
        true => Ok(map),
        false => Err(KeymapParserError::Invalid.spanned(span)),
    }
}
