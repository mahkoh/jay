use crate::config::Action;
use crate::config::Shortcut;
use crate::config::SimpleCommand;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::arr;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::input_mode::InputMode;
use crate::config::parsers::modified_keysym::ModifiedKeysymParser;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use ahash::AHashMap;
use indexmap::IndexMap;
use jay_config::keyboard::ModifiedKeySym;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InhibitShortcutsParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct InhibitShortcutsParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

#[derive(Clone, Debug, Default)]
pub struct InhibitShortcuts {
    pub enabled: Option<bool>,
}

impl Parser for InhibitShortcutsParser<'_, '_, '_> {
    type Value = InhibitShortcuts;
    type Error = InhibitShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            enabled, //
        ) = ext.extract((
            opt(bol("enabled")), //
        ))?;
        Ok(InhibitShortcuts {
            enabled: enabled.despan(),
        })
    }
}

pub struct InhibitShortcutsWhitelistParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

impl Parser for InhibitShortcutsWhitelistParser<'_, '_, '_> {
    type Value = Vec<Spanned<ModifiedKeySym>>;
    type Error = InhibitShortcutsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            keys, //
        ) = ext.extract((
            recover(opt(arr("keys"))), //
        ))?;
        let mut res = vec![];
        if let Some(keys) = keys {
            for el in keys.value {
                let el_span = el.span;
                match el.parse(&mut ModifiedKeysymParser) {
                    Ok(keysym) => res.push(keysym.spanned(el_span)),
                    Err(e) => {
                        log::warn!(
                            "Could not parse whitelisted shortcut key: {}",
                            self.0.error(e),
                        );
                    }
                }
            }
        }
        Ok(res)
    }
}

pub fn apply_inhibit_shortcuts_whitelist(
    cx: &Context<'_, '_>,
    keys: &[Spanned<ModifiedKeySym>],
    shortcuts: &mut [Shortcut],
    input_modes: &mut AHashMap<String, InputMode>,
) {
    for key in keys {
        let mut matched = whitelist_key_in_shortcuts(shortcuts, key.value);
        for mode in input_modes.values_mut() {
            if whitelist_key_in_shortcuts(&mut mode.shortcuts, key.value) {
                matched = true;
            }
        }
        if !matched {
            log::warn!(
                "Whitelisted shortcut is not defined in [shortcuts]: {}",
                cx.error3(key.span),
            );
        }
    }
}

fn whitelist_key_in_shortcuts(shortcuts: &mut [Shortcut], keysym: ModifiedKeySym) -> bool {
    let mut active = false;
    for shortcut in shortcuts.iter_mut() {
        if shortcut.keysym != keysym {
            continue;
        }
        if is_shortcut_active(shortcut) {
            shortcut.allow_inhibited = true;
            active = true;
        } else {
            active = false;
        }
    }
    active
}

fn is_shortcut_active(shortcut: &Shortcut) -> bool {
    !matches!(
        shortcut.action,
        Action::SimpleCommand {
            cmd: SimpleCommand::None,
        }
    ) || shortcut.latch.is_some()
}
