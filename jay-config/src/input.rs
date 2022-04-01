pub mod acceleration;
pub mod capability;

use crate::input::acceleration::AccelProfile;
use crate::input::capability::Capability;
use crate::Seat;
use bincode::{Decode, Encode};

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct InputDevice(pub u64);

impl InputDevice {
    pub fn set_seat(self, seat: Seat) {
        get!().set_seat(self, seat)
    }

    pub fn has_capability(self, cap: Capability) -> bool {
        get!(false).has_capability(self, cap)
    }

    pub fn set_left_handed(self, left_handed: bool) {
        get!().set_left_handed(self, left_handed);
    }

    pub fn set_accel_profile(self, profile: AccelProfile) {
        get!().set_accel_profile(self, profile);
    }

    pub fn set_accel_speed(self, speed: f64) {
        get!().set_accel_speed(self, speed);
    }

    pub fn set_transform_matrix(self, matrix: [[f64; 2]; 2]) {
        get!().set_transform_matrix(self, matrix);
    }

    pub fn name(self) -> String {
        get!(String::new()).device_name(self)
    }
}
