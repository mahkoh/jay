use bincode::{Decode, Encode};
use crate::Seat;

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct InputDevice(pub u64);

impl InputDevice {
    pub fn set_seat(self, seat: Seat) {
        get!().set_seat(self, seat)
    }

    pub fn has_capability(self, cap: Capability) -> bool {
        get!(false).has_capability(self, cap)
    }
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Capability(pub u32);

pub const CAP_KEYBOARD: Capability = Capability(0);
pub const CAP_POINTER: Capability = Capability(1);
pub const CAP_TOUCH: Capability = Capability(2);
pub const CAP_TABLET_TOOL: Capability = Capability(3);
pub const CAP_TABLET_PAD: Capability = Capability(4);
pub const CAP_GESTURE: Capability = Capability(5);
pub const CAP_SWITCH: Capability = Capability(6);
