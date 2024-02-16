//! Constants determining the acceleration profile of a device.
//!
//! See the libinput documentation for details.

use serde::{Deserialize, Serialize};

/// The acceleration profile of a device.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct AccelProfile(pub u32);

/// A flat acceleration profile.
pub const ACCEL_PROFILE_FLAT: AccelProfile = AccelProfile(1 << 0);
/// An adaptive acceleration profile.
pub const ACCEL_PROFILE_ADAPTIVE: AccelProfile = AccelProfile(1 << 1);
