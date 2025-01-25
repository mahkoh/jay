//! Tools affecting the keyboard behavior.

use {
    crate::keyboard::{mods::Modifiers, syms::KeySym},
    serde::{Deserialize, Serialize},
    std::{
        fmt,
        ops::{BitOr, BitOrAssign},
    },
};

pub mod mods;
pub mod syms;

#[derive(Serialize, Deserialize, Clone, Eq, Hash, Debug)]
pub struct AppMod {
    pub app_name: String,
    pub mod_name: String,
}

const APP_NAME_JAY: &str = "Jay";
const MOD_NAME_GLOBAL: &str = "Global";
const MOD_NAME_WINDOW: &str = "Window";
const MOD_NAME_INSERT: &str = "Insert";
const MOD_NAME_INITIAL: &str = "Init";

impl AppMod {
    pub const APP_NAME_JAY: &str = APP_NAME_JAY;
    pub const MOD_NAME_GLOBAL: &str = MOD_NAME_GLOBAL;
    pub const MOD_NAME_WINDOW: &str = MOD_NAME_WINDOW;
    pub const MOD_NAME_INSERT: &str = MOD_NAME_INSERT;
    pub const MOD_NAME_INITIAL: &str = MOD_NAME_INITIAL;

    pub fn is_global(&self) -> bool {
        let Self { app_name, mod_name } = self;
        app_name == APP_NAME_JAY && mod_name == MOD_NAME_GLOBAL
    }
    pub fn global() -> Self {
        Self {
            app_name: APP_NAME_JAY.to_string(),
            mod_name: MOD_NAME_GLOBAL.to_string(),
        }
    }
    pub fn is_window(&self) -> bool {
        let Self { app_name, mod_name } = self;
        app_name == APP_NAME_JAY && mod_name == MOD_NAME_WINDOW
    }
    pub fn is_insert(&self) -> bool {
        let Self { app_name, mod_name } = self;
        app_name == APP_NAME_JAY && mod_name == MOD_NAME_INSERT
    }
    pub fn insert() -> Self {
        Self {
            app_name: APP_NAME_JAY.to_string(),
            mod_name: MOD_NAME_INSERT.to_string(),
        }
    }
    pub fn as_tuple(self) -> (String, String) {
        let Self { app_name, mod_name } = self;
        (app_name, mod_name)
    }
}

impl fmt::Display for AppMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let AppMod { app_name, mod_name } = self;
        write!(f, r#"AppMod("{}", "{}")"#, app_name, mod_name)
    }
}

impl Default for AppMod {
    fn default() -> Self {
        Self {
            app_name: APP_NAME_JAY.to_string(),
            mod_name: MOD_NAME_WINDOW.to_string(),
        }
    }
}

impl PartialEq for AppMod {
    fn eq(&self, other: &Self) -> bool {
        self.app_name == other.app_name && self.mod_name == other.mod_name
    }
    fn ne(&self, other: &Self) -> bool {
        self.app_name != other.app_name || self.mod_name != other.mod_name
    }
}

impl From<&(String, String)> for AppMod {
    fn from(app_mod_key: &(String, String)) -> Self {
        Self {
            app_name: app_mod_key.0.clone(),
            mod_name: app_mod_key.1.clone(),
        }
    }
}

/// A keysym with zero or more modifiers
#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ModifiedKeySym {
    pub mods: Modifiers,
    pub sym: KeySym,
}

impl From<KeySym> for ModifiedKeySym {
    fn from(sym: KeySym) -> Self {
        Self {
            mods: Modifiers(0),
            sym,
        }
    }
}

impl BitOr<Modifiers> for ModifiedKeySym {
    type Output = ModifiedKeySym;

    fn bitor(self, rhs: Modifiers) -> Self::Output {
        ModifiedKeySym {
            mods: self.mods | rhs,
            sym: self.sym,
        }
    }
}

impl BitOrAssign<Modifiers> for ModifiedKeySym {
    fn bitor_assign(&mut self, rhs: Modifiers) {
        self.mods |= rhs;
    }
}

/// A keymap.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Keymap(pub u64);

impl Keymap {
    /// The invalid keymap.
    pub const INVALID: Self = Self(0);

    /// Returns whether this keymap is valid.
    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }

    /// Returns whether this keymap is invalid.
    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }

    /// Destroys this reference to the keymap.
    ///
    /// Seats that are currently using this keymap are unaffected.
    pub fn destroy(self) {
        if self.is_valid() {
            get!().destroy_keymap(self);
        }
    }
}

/// Parses a keymap.
///
/// The returned keymap can later be used to set the keymap of a seat. If the keymap cannot
/// be parsed, returns an invalid keymap. Trying to set a seat's keymap to an invalid keymap
/// has no effect.
///
/// A simple keymap looks as follows:
///
/// ```text
/// xkb_keymap {
///     xkb_keycodes  { include "evdev+aliases(qwerty)" };
///     xkb_types     { include "complete" };
///     xkb_compat    { include "complete" };
///     xkb_symbols   { include "pc+inet(evdev)+us(basic)" };
/// };
/// ```
///
/// To use a programmer Dvorak, replace the corresponding line by
///
/// ```text
///     xkb_symbols   { include "pc+inet(evdev)+us(dvp)" };
/// ```
///
/// To use a German keymap, replace the corresponding line by
///
/// ```text
///     xkb_symbols   { include "pc+inet(evdev)+de(basic)" };
/// ```
///
/// You can also use a completely custom keymap that doesn't use any includes. See the
/// [default][default] Jay keymap for an example.
///
/// General information about the keymap format can be found in the [arch wiki][wiki].
///
/// [default]: https://github.com/mahkoh/jay/tree/master/src/keymap.xkb
/// [wiki]: https://wiki.archlinux.org/title/X_keyboard_extension
pub fn parse_keymap(keymap: &str) -> Keymap {
    get!(Keymap::INVALID).parse_keymap(keymap)
}
