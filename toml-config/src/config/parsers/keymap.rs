use {
    crate::{
        config::{
            ConfigKeymap,
            context::Context,
            extractor::{Extractor, ExtractorError, opt, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::{
        config_dir,
        keyboard::{Keymap, keymap_from_names, parse_keymap},
    },
    kbvm::xkb::rmlvo::Group,
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
    #[error("Keymap table must contain at least one of `name`, `map`, `path`, `rmlvo`")]
    MissingField,
    #[error(
        "Keymap must have both `name` and one of `map`, `path`, `rmlvo` fields in this context"
    )]
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
        let (mut name_val, mut map_val, mut path, mut rmlvo) = ext.extract((
            opt(str("name")),
            opt(str("map")),
            opt(str("path")),
            opt(val("rmlvo")),
        ))?;
        if map_val.is_some() as u32 + path.is_some() as u32 + rmlvo.is_some() as u32 > 1 {
            log::warn!(
                "At most one of `map`, `path`, and `rmlvo` should be specified: {}",
                self.cx.error3(span),
            );
            let ignore_path = map_val.is_some();
            let ignore_rmlvo = map_val.is_some() || path.is_some();
            if ignore_path && path.is_some() {
                log::warn!("Ignoring `path`");
                path = None;
            }
            if ignore_rmlvo && rmlvo.is_some() {
                log::warn!("Ignoring `rmlvo`");
                rmlvo = None;
            }
        }
        let file_content;
        if let Some(path) = path {
            let mut root = PathBuf::from(config_dir());
            root.push(path.value);
            file_content = match std::fs::read_to_string(&root) {
                Ok(c) => c,
                Err(e) => {
                    return Err(KeymapParserError::ReadFile(root.display().to_string(), e)
                        .spanned(path.span));
                }
            };
            map_val = Some(file_content.as_str().spanned(path.span));
        }
        let mut map = None;
        if let Some(val) = &map_val {
            map = Some(parse(val.span, val.value)?);
        }
        if let Some(val) = rmlvo {
            map = Some(val.parse(&mut RmlvoParser(self.cx))?);
        }
        if self.definition && (name_val.is_none() || map.is_none()) {
            return Err(KeymapParserError::DefinitionRequired.spanned(span));
        }
        if !self.definition && map.is_some() {
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
        let res = match (name_val, map) {
            (Some(name_val), Some(map)) => ConfigKeymap::Defined {
                name: name_val.value.to_string(),
                map,
            },
            (Some(name_val), None) => ConfigKeymap::Named(name_val.value.to_string()),
            (None, Some(map)) => ConfigKeymap::Literal(map),
            (None, None) => return Err(KeymapParserError::MissingField.spanned(span)),
        };
        Ok(res)
    }
}

struct RmlvoParser<'a>(&'a Context<'a>);

impl Parser for RmlvoParser<'_> {
    type Value = Keymap;
    type Error = KeymapParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (rules, model, layout, variants, options) = ext.extract((
            opt(str("rules")),
            opt(str("model")),
            opt(str("layout")),
            opt(str("variants")),
            opt(str("options")),
        ))?;
        let mut groups = None::<Vec<_>>;
        if layout.is_some() || variants.is_some() {
            groups = Some(
                Group::from_layouts_and_variants(
                    layout.despan().unwrap_or_default(),
                    variants.despan().unwrap_or_default(),
                )
                .map(|g| jay_config::keyboard::Group {
                    layout: g.layout,
                    variant: g.variant,
                })
                .collect(),
            );
        }
        let mut options_vec = None::<Vec<_>>;
        if let Some(options) = options {
            options_vec = Some(options.value.split(",").collect());
        }
        let map = keymap_from_names(
            rules.despan(),
            model.despan(),
            groups.as_deref(),
            options_vec.as_deref(),
        );
        match map.is_valid() {
            true => Ok(map),
            false => Err(KeymapParserError::Invalid.spanned(span)),
        }
    }
}

fn parse(span: Span, string: &str) -> Result<Keymap, Spanned<KeymapParserError>> {
    let map = parse_keymap(string);
    match map.is_valid() {
        true => Ok(map),
        false => Err(KeymapParserError::Invalid.spanned(span)),
    }
}
