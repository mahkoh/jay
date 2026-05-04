//! Tools affecting the keyboard behavior.

use {
    crate::{
        _private::{KeymapBuildParamsV1, KeymapBuildParamsV1Kind},
        keyboard::{mods::Modifiers, syms::KeySym},
    },
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

    /// Creates a keymap builder.
    pub fn builder<'a>() -> KeymapBuilder<'a> {
        KeymapBuilder {
            v1: KeymapBuildParamsV1 {
                kind: None,
                shortcuts_group: None,
            },
        }
    }
}

/// An RMLVO group consisting of a layout and a variant.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Group<'a> {
    /// The layout of the group.
    pub layout: &'a str,
    /// The variant of the group. Can be an empty string.
    pub variant: &'a str,
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
/// See also [`Keymap::builder`] for a more general interface.
///
/// [default]: https://github.com/mahkoh/jay/tree/master/src/keymap.xkb
/// [wiki]: https://wiki.archlinux.org/title/X_keyboard_extension
pub fn parse_keymap(keymap: &str) -> Keymap {
    get!(Keymap::INVALID).parse_keymap(keymap)
}

/// Creates a keymap from RMLVO names.
///
/// If a parameter is not given, a value from the environment or a default is used:
///
/// | name                   | default                |
/// | ---------------------- | ---------------------- |
/// | `XKB_DEFAULT_RULES`    | `evdev`                |
/// | `XKB_DEFAULT_MODEL`    | `pc105`                |
/// | `XKB_DEFAULT_LAYOUT`   | `us`                   |
/// | `XKB_DEFAULT_VARIANTS` |                        |
/// | `XKB_DEFAULT_OPTIONS`  |                        |
///
/// `XKB_DEFAULT_LAYOUT` and `XKB_DEFAULT_VARIANTS` are parsed into the `groups` parameter like this example:
/// ```
/// XKB_DEFAULT_LAYOUT = "us,il,ru,de,jp"
/// XKB_DEFAULT_VARIANTS = ",,phonetic,neo"
/// ```
/// produces:
/// ```
/// [
///     Group { layout: "us", variant: "" },
///     Group { layout: "il", variant: "" },
///     Group { layout: "ru", variant: "phonetic" },
///     Group { layout: "de", variant: "neo" },
///     Group { layout: "jp", variant: "" },
/// ]
/// ```
///
/// See also [`Keymap::builder`] for a more general interface.
pub fn keymap_from_names(
    rules: Option<&str>,
    model: Option<&str>,
    groups: Option<&[Group<'_>]>,
    options: Option<&[&str]>,
) -> Keymap {
    get!(Keymap::INVALID).keymap_from_names(rules, model, groups, options)
}

/// A keymap builder.
pub struct KeymapBuilder<'a> {
    pub(crate) v1: KeymapBuildParamsV1<'a>,
}

impl<'a> KeymapBuilder<'a> {
    /// Builds the keymap.
    pub fn build(self) -> Keymap {
        get!(Keymap::INVALID).parse_keymap_2(self)
    }

    /// Sets the XKB map to parse.
    ///
    /// See [`parse_keymap`] for details about the format.
    pub fn map(mut self, map: &'a str) -> Self {
        self.v1.kind = Some(KeymapBuildParamsV1Kind::Map(map));
        self
    }

    /// Sets the RMLVO names to parse.
    ///
    /// See [`keymap_from_names`] for details about the format.
    pub fn names(
        mut self,
        rules: Option<&'a str>,
        model: Option<&'a str>,
        groups: Option<&'a [Group<'a>]>,
        options: Option<&'a [&'a str]>,
    ) -> Self {
        self.v1.kind = Some(KeymapBuildParamsV1Kind::Names {
            rules,
            model,
            groups: groups.map(|v| v.to_vec()),
            options: options.map(|v| v.to_vec()),
        });
        self
    }

    /// Sets the 0-based group index to use for shortcuts.
    ///
    /// By default, the group that is active at the time of the key press is used. For
    /// example, if the keymap has two groups, US and RU, and the active group is 1,
    /// shortcuts are looked up against the Russian group.
    ///
    /// By calling `shortcuts_group(0)`, shortcuts are instead always looked up against
    /// the English group.
    pub fn shortcuts_group(mut self, group: u32) -> Self {
        self.v1.shortcuts_group = Some(group);
        self
    }
}
