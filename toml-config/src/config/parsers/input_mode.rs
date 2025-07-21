use {
    crate::{
        config::{
            Shortcut,
            context::Context,
            extractor::{Extractor, ExtractorError, opt, recover, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::shortcuts::{ComplexShortcutsParser, ShortcutsParser, ShortcutsParserError},
            spanned::SpannedErrorExt,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    ahash::{AHashMap, AHashSet},
    indexmap::IndexMap,
    std::collections::HashSet,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum InputModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    ExtractorError(#[from] ExtractorError),
    #[error("Could not parse the shortcuts")]
    ParseShortcuts(#[source] ShortcutsParserError),
}

#[derive(Clone, Debug)]
pub struct InputMode {
    pub parent: Option<String>,
    pub shortcuts: Vec<Shortcut>,
}

pub struct InputModesParser<'a>(pub &'a Context<'a>);

impl Parser for InputModesParser<'_> {
    type Value = AHashMap<String, InputMode>;
    type Error = InputModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut modes = AHashMap::new();
        let mut used = AHashSet::new();
        for (key, value) in table.iter() {
            let mode = match value.parse(&mut InputModeParser(self.0)) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!(
                        "Could not parse input mode {}: {}",
                        key.value,
                        self.0.error(e)
                    );
                    continue;
                }
            };
            log_used(self.0, &mut used, key);
            modes.insert(key.value.to_string(), mode);
        }
        Ok(modes)
    }
}

pub struct InputModeParser<'a>(pub &'a Context<'a>);

impl Parser for InputModeParser<'_> {
    type Value = InputMode;
    type Error = InputModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (parent, shortcuts_val, complex_shortcuts_val) = ext.extract((
            recover(opt(str("parent"))),
            opt(val("shortcuts")),
            opt(val("complex-shortcuts")),
        ))?;
        let mut used_keys = HashSet::new();
        let mut shortcuts = vec![];
        if let Some(value) = shortcuts_val {
            value
                .parse(&mut ShortcutsParser {
                    cx: self.0,
                    used_keys: &mut used_keys,
                    shortcuts: &mut shortcuts,
                })
                .map_spanned_err(InputModeParserError::ParseShortcuts)?;
        }
        if let Some(value) = complex_shortcuts_val {
            value
                .parse(&mut ComplexShortcutsParser {
                    cx: self.0,
                    used_keys: &mut used_keys,
                    shortcuts: &mut shortcuts,
                })
                .map_spanned_err(InputModeParserError::ParseShortcuts)?;
        }
        Ok(InputMode {
            parent: parent.despan_into(),
            shortcuts,
        })
    }
}

fn log_used(cx: &Context<'_>, used: &mut AHashSet<Spanned<String>>, key: &Spanned<String>) {
    if let Some(prev) = used.get(key) {
        log::warn!(
            "Duplicate input mode overrides previous definition: {}",
            cx.error3(key.span)
        );
        log::info!("Previous definition here: {}", cx.error3(prev.span));
    }
    used.insert(key.clone());
}
