//! Keyboard modifiers

use {
    crate::{keyboard::syms::KeySym, ModifiedKeySym},
    serde::{Deserialize, Serialize},
    std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign},
};

/// Zero or more keyboard modifiers
#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Default, Hash, Debug)]
pub struct Modifiers(pub u32);

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Modifiers(0);
}

/// The Shift modifier
pub const SHIFT: Modifiers = Modifiers(1 << 0);
/// The CapsLock modifier.
pub const LOCK: Modifiers = Modifiers(1 << 1);
/// The Ctrl modifier.
pub const CTRL: Modifiers = Modifiers(1 << 2);
/// The Mod1 modifier, i.e., Alt.
pub const MOD1: Modifiers = Modifiers(1 << 3);
/// The Mod2 modifier, i.e., NumLock.
pub const MOD2: Modifiers = Modifiers(1 << 4);
/// The Mod3 modifier.
pub const MOD3: Modifiers = Modifiers(1 << 5);
/// The Mod4 modifier, i.e., Logo.
pub const MOD4: Modifiers = Modifiers(1 << 6);
/// The Mod5 modifier.
pub const MOD5: Modifiers = Modifiers(1 << 7);

/// Alias for `LOCK`.
pub const CAPS: Modifiers = LOCK;
/// Alias for `MOD1`.
pub const ALT: Modifiers = MOD1;
/// Alias for `MOD2`.
pub const NUM: Modifiers = MOD2;
/// Alias for `MOD4`.
pub const LOGO: Modifiers = MOD4;

/// Synthetic modifier matching key release events.
///
/// This can be used to execute a callback on key release.
pub const RELEASE: Modifiers = Modifiers(1 << 31);

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOr<KeySym> for Modifiers {
    type Output = ModifiedKeySym;

    fn bitor(self, rhs: KeySym) -> Self::Output {
        ModifiedKeySym {
            mods: self,
            sym: rhs,
        }
    }
}

impl BitAnd for Modifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

impl BitAndAssign for Modifiers {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0
    }
}
