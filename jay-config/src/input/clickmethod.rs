//! Constants determining the click method of a device.
//!
//! See the libinput documentation for details.

use serde::{Deserialize, Serialize};

/// The click method of a device.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ClickMethod(pub u32);

/// No click method handling
pub const CLICK_METHOD_NONE: ClickMethod = ClickMethod(0);

/// Button area
pub const CLICK_METHOD_BUTTON_AREAS: ClickMethod = ClickMethod(1 << 0);

/// Clickfinger
pub const CLICK_METHOD_CLICKFINGER: ClickMethod = ClickMethod(1 << 1);
