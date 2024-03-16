//! Tools affecting the keyboard behavior.

use {
    crate::keyboard::{mods::Modifiers, syms::KeySym},
    serde::{Deserialize, Serialize},
    std::ops::{BitOr, BitOrAssign},
};

pub mod mods;
pub mod syms;

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
