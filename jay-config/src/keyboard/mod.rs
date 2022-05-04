use {
    crate::keyboard::{mods::Modifiers, syms::KeySym},
    bincode::{Decode, Encode},
    std::ops::{BitOr, BitOrAssign},
};

pub mod keymap;
pub mod mods;
pub mod syms;

#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Hash, Debug)]
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
