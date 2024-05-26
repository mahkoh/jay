use {
    crate::{
        config::{
            context::Context,
            extractor::{opt, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::{ActionParser, ActionParserError},
                modified_keysym::{
                    ModifiedKeysymParser, ModifiedKeysymParserError, ModifiersParser,
                },
            },
            spanned::SpannedErrorExt,
            Action, Shortcut, SimpleCommand,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::keyboard::{mods::Modifiers, ModifiedKeySym},
    std::collections::HashSet,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ShortcutsParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    ExtractorError(#[from] ExtractorError),
    #[error("Could not parse the mod mask")]
    ModMask(#[source] ModifiedKeysymParserError),
    #[error("Could not parse the action")]
    ActionParserError(#[source] ActionParserError),
    #[error("Could not parse the latch action")]
    LatchError(#[source] ActionParserError),
}

pub struct ShortcutsParser<'a, 'b> {
    pub cx: &'a Context<'a>,
    pub used_keys: &'b mut HashSet<Spanned<ModifiedKeySym>>,
    pub shortcuts: &'b mut Vec<Shortcut>,
}

impl Parser for ShortcutsParser<'_, '_> {
    type Value = ();
    type Error = ShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        for (key, value) in table.iter() {
            let Some(keysym) = parse_modified_keysym(self.cx, key) else {
                continue;
            };
            let Some(action) = parse_action(self.cx, &key.value, value) else {
                continue;
            };
            let spanned = keysym.spanned(key.span);
            log_used(self.cx, self.used_keys, spanned);
            self.shortcuts.push(Shortcut {
                mask: Modifiers(!0),
                keysym,
                action,
                latch: None,
            });
        }
        Ok(())
    }
}

pub struct ComplexShortcutsParser<'a, 'b> {
    pub cx: &'a Context<'a>,
    pub used_keys: &'b mut HashSet<Spanned<ModifiedKeySym>>,
    pub shortcuts: &'b mut Vec<Shortcut>,
}

impl Parser for ComplexShortcutsParser<'_, '_> {
    type Value = ();
    type Error = ShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        for (key, value) in table.iter() {
            let Some(keysym) = parse_modified_keysym(self.cx, key) else {
                continue;
            };
            let shortcut = match value.parse(&mut ComplexShortcutParser {
                keysym,
                cx: self.cx,
            }) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(
                        "Could not parse shortcut for keysym {}: {}",
                        key.value,
                        self.cx.error(e)
                    );
                    continue;
                }
            };
            let spanned = keysym.spanned(key.span);
            log_used(self.cx, self.used_keys, spanned);
            self.shortcuts.push(shortcut);
        }
        Ok(())
    }
}

struct ComplexShortcutParser<'a> {
    pub keysym: ModifiedKeySym,
    pub cx: &'a Context<'a>,
}

impl Parser for ComplexShortcutParser<'_> {
    type Value = Shortcut;
    type Error = ShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (mod_mask_val, action_val, latch_val) =
            ext.extract((opt(str("mod-mask")), opt(val("action")), opt(val("latch"))))?;
        let mod_mask = match mod_mask_val {
            None => Modifiers(!0),
            Some(v) => ModifiersParser
                .parse_string(v.span, v.value)
                .map_spanned_err(ShortcutsParserError::ModMask)?,
        };
        let action = match action_val {
            None => Action::SimpleCommand {
                cmd: SimpleCommand::None,
            },
            Some(v) => v
                .parse(&mut ActionParser(self.cx))
                .map_spanned_err(ShortcutsParserError::ActionParserError)?,
        };
        let mut latch = None;
        if let Some(v) = latch_val {
            latch = Some(
                v.parse(&mut ActionParser(self.cx))
                    .map_spanned_err(ShortcutsParserError::LatchError)?,
            );
        }
        Ok(Shortcut {
            mask: mod_mask,
            keysym: self.keysym,
            action,
            latch,
        })
    }
}

fn parse_action(cx: &Context<'_>, key: &str, value: &Spanned<Value>) -> Option<Action> {
    match value.parse(&mut ActionParser(cx)) {
        Ok(a) => Some(a),
        Err(e) => {
            log::warn!("Could not parse action for keysym {key}: {}", cx.error(e));
            None
        }
    }
}

fn parse_modified_keysym(cx: &Context<'_>, key: &Spanned<String>) -> Option<ModifiedKeySym> {
    parse_modified_keysym_str(cx, key.span, &key.value)
}

pub fn parse_modified_keysym_str(
    cx: &Context<'_>,
    span: Span,
    value: &str,
) -> Option<ModifiedKeySym> {
    match ModifiedKeysymParser.parse_string(span, value) {
        Ok(k) => Some(k),
        Err(e) => {
            log::warn!("Could not parse keysym {}: {}", value, cx.error(e));
            None
        }
    }
}

fn log_used(
    cx: &Context<'_>,
    used: &mut HashSet<Spanned<ModifiedKeySym>>,
    key: Spanned<ModifiedKeySym>,
) {
    if let Some(prev) = used.get(&key) {
        log::warn!(
            "Duplicate key overrides previous definition: {}",
            cx.error3(key.span)
        );
        log::info!("Previous definition here: {}", cx.error3(prev.span));
    }
    used.insert(key);
}
