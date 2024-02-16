//! Constants specifying the capabilities of an input device.
//!
//! See the libinput documentation for the meanings of these constants.

use serde::{Deserialize, Serialize};

/// A capability of an input device.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Capability(pub u32);

pub const CAP_KEYBOARD: Capability = Capability(0);
pub const CAP_POINTER: Capability = Capability(1);
pub const CAP_TOUCH: Capability = Capability(2);
pub const CAP_TABLET_TOOL: Capability = Capability(3);
pub const CAP_TABLET_PAD: Capability = Capability(4);
pub const CAP_GESTURE: Capability = Capability(5);
pub const CAP_SWITCH: Capability = Capability(6);
