use {
    crate::{
        config::{
            keysyms::KEYSYMS,
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::toml_span::{Span, SpannedExt},
    },
    jay_config::keyboard::{
        ModifiedKeySym,
        mods::{
            ALT, CAPS, CTRL, LOCK, LOGO, MOD1, MOD2, MOD3, MOD4, MOD5, Modifiers, NUM, RELEASE,
            SHIFT,
        },
    },
    thiserror::Error,
};

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
                _ => match KEYSYMS.get(part) {
                    Some(new) if sym.is_none() => {
                        sym = Some(*new);
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
