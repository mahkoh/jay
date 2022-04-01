use bincode::{Decode, Encode};

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct AccelProfile(pub u32);

pub const ACCEL_PROFILE_FLAT: AccelProfile = AccelProfile(1 << 0);
pub const ACCEL_PROFILE_ADAPTIVE: AccelProfile = AccelProfile(1 << 1);
