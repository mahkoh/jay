//! Keyboard modifiers

use {
    crate::{ModifiedKeySym, keyboard::syms::KeySym},
    serde::{Deserialize, Serialize},
    std::ops::BitOr,
};

bitflags! {
    /// Zero or more keyboard modifiers
    #[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Default, Hash)]
    pub struct Modifiers(pub u32) {
        /// The Shift modifier
        pub const SHIFT = 1 << 0,
        /// The CapsLock modifier.
        pub const LOCK = 1 << 1,
        /// The Ctrl modifier.
        pub const CTRL = 1 << 2,
        /// The Mod1 modifier, i.e., Alt.
        pub const MOD1 = 1 << 3,
        /// The Mod2 modifier, i.e., NumLock.
        pub const MOD2 = 1 << 4,
        /// The Mod3 modifier.
        pub const MOD3 = 1 << 5,
        /// The Mod4 modifier, i.e., Logo.
        pub const MOD4 = 1 << 6,
        /// The Mod5 modifier.
        pub const MOD5 = 1 << 7,

        /// Synthetic modifier matching key release events.
        ///
        /// This can be used to execute a callback on key release.
        pub const RELEASE = 1 << 31,
    }
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Modifiers(0);
}

/// Alias for `LOCK`.
pub const CAPS: Modifiers = LOCK;
/// Alias for `MOD1`.
pub const ALT: Modifiers = MOD1;
/// Alias for `MOD2`.
pub const NUM: Modifiers = MOD2;
/// Alias for `MOD4`.
pub const LOGO: Modifiers = MOD4;

impl BitOr<KeySym> for Modifiers {
    type Output = ModifiedKeySym;

    fn bitor(self, rhs: KeySym) -> Self::Output {
        ModifiedKeySym {
            mods: self,
            sym: rhs,
        }
    }
}
