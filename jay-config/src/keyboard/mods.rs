use {
    crate::{keyboard::syms::KeySym, ModifiedKeySym},
    bincode::{Decode, Encode},
    std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign},
};

#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Default, Hash, Debug)]
pub struct Modifiers(pub u32);

pub const SHIFT: Modifiers = Modifiers(1 << 0);
pub const LOCK: Modifiers = Modifiers(1 << 1);
pub const CTRL: Modifiers = Modifiers(1 << 2);
pub const MOD1: Modifiers = Modifiers(1 << 3);
pub const MOD2: Modifiers = Modifiers(1 << 4);
pub const MOD3: Modifiers = Modifiers(1 << 5);
pub const MOD4: Modifiers = Modifiers(1 << 6);
pub const MOD5: Modifiers = Modifiers(1 << 7);

pub const CAPS: Modifiers = LOCK;
pub const ALT: Modifiers = MOD1;
pub const NUM: Modifiers = MOD2;
pub const LOGO: Modifiers = MOD4;

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
