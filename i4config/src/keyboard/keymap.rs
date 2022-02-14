use bincode::{Decode, Encode};

#[derive(Encode, Decode, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Keymap(pub u64);

impl Keymap {
    pub const INVALID: Self = Self(0);

    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }

    pub fn parse(self, keymap: &str) -> Self {
        let mut res = Self::INVALID;
        (|| res = get!().parse_keymap(keymap))();
        res
    }
}
