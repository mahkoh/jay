use {
    crate::{
        config::{
            context::Context,
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{action::ActionParser, modified_keysym::ModifiedKeysymParser},
            Action,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::keyboard::ModifiedKeySym,
    std::collections::HashSet,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ShortcutsParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
}

pub struct ShortcutsParser<'a>(pub &'a Context<'a>);

impl Parser for ShortcutsParser<'_> {
    type Value = Vec<(ModifiedKeySym, Action)>;
    type Error = ShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut used_keys = HashSet::<Spanned<ModifiedKeySym>>::new();
        let mut res = vec![];
        for (key, value) in table.iter() {
            let keysym = match ModifiedKeysymParser.parse_string(key.span, &key.value) {
                Ok(k) => k,
                Err(e) => {
                    log::warn!("Could not parse keysym: {}", self.0.error(e));
                    continue;
                }
            };
            let action = match value.parse(&mut ActionParser(self.0)) {
                Ok(a) => a,
                Err(e) => {
                    log::warn!(
                        "Could not parse action for keysym {}: {}",
                        key.value,
                        self.0.error(e)
                    );
                    continue;
                }
            };
            let spanned = keysym.spanned(key.span);
            if let Some(prev) = used_keys.get(&spanned) {
                log::warn!(
                    "Duplicate key overrides previous definition: {}",
                    self.0.error3(spanned.span)
                );
                log::info!("Previous definition here: {}", self.0.error3(prev.span));
            }
            used_keys.insert(spanned);
            res.push((keysym, action));
        }
        Ok(res)
    }
}
