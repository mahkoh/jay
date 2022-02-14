use crate::keyboard::mods::Modifiers;
use crate::keyboard::syms::KeySym;
use bincode::{Decode, Encode};
use std::ops::{BitOr, BitOrAssign};

pub mod mods;
pub mod syms;
pub mod keymap;

#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Hash)]
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
