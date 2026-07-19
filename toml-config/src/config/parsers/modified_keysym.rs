use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::SpannedExt;
use jay_config::keyboard::ModifiedKeySym;
use jay_config::keyboard::mods::ALT;
use jay_config::keyboard::mods::CAPS;
use jay_config::keyboard::mods::CTRL;
use jay_config::keyboard::mods::LOCK;
use jay_config::keyboard::mods::LOGO;
use jay_config::keyboard::mods::MOD1;
use jay_config::keyboard::mods::MOD2;
use jay_config::keyboard::mods::MOD3;
use jay_config::keyboard::mods::MOD4;
use jay_config::keyboard::mods::MOD5;
use jay_config::keyboard::mods::Modifiers;
use jay_config::keyboard::mods::NUM;
use jay_config::keyboard::mods::RELEASE;
use jay_config::keyboard::mods::SHIFT;
use jay_config::keyboard::syms::KeySym;
use kbvm::Keysym;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModifiedKeysymParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("You cannot use more than one non-modifier key")]
    MoreThanOneSym,
    #[error("You must specify exactly one non-modifier key")]
    MissingSym,
    #[error("Unknown keysym {0}")]
    UnknownKeysym(String),
    #[error("Unknown modifier {0}")]
    UnknownModifier(String),
}

pub struct ModifiedKeysymParser;

impl Parser for ModifiedKeysymParser {
    type Value = ModifiedKeySym;
    type Error = ModifiedKeysymParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mut modifiers = Modifiers(0);
        let mut sym = None;
        for part in string.split("-") {
            let modifier = match parse_mod(part) {
                Some(m) => m,
                _ => match Keysym::from_str(part) {
                    Some(new) if sym.is_none() => {
                        sym = Some(KeySym(new.0));
                        continue;
                    }
                    Some(_) => return Err(ModifiedKeysymParserError::MoreThanOneSym.spanned(span)),
                    _ => {
                        return Err(ModifiedKeysymParserError::UnknownKeysym(part.to_string())
                            .spanned(span));
                    }
                },
            };
            modifiers |= modifier;
        }
        match sym {
            Some(s) => Ok(modifiers | s),
            None => Err(ModifiedKeysymParserError::MissingSym.spanned(span)),
        }
    }
}

pub struct ModifiersParser;

impl Parser for ModifiersParser {
    type Value = Modifiers;
    type Error = ModifiedKeysymParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let mut modifiers = Modifiers(0);
        if !string.is_empty() {
            for part in string.split("-") {
                let Some(modifier) = parse_mod(part) else {
                    return Err(
                        ModifiedKeysymParserError::UnknownModifier(part.to_string()).spanned(span)
                    );
                };
                modifiers |= modifier;
            }
        }
        Ok(modifiers)
    }
}

fn parse_mod(part: &str) -> Option<Modifiers> {
    let modifier = match part {
        "shift" => SHIFT,
        "lock" => LOCK,
        "ctrl" => CTRL,
        "mod1" => MOD1,
        "mod2" => MOD2,
        "mod3" => MOD3,
        "mod4" => MOD4,
        "mod5" => MOD5,
        "caps" => CAPS,
        "alt" => ALT,
        "num" => NUM,
        "logo" => LOGO,
        "release" => RELEASE,
        _ => return None,
    };
    Some(modifier)
}
