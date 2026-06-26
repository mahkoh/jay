//! Constants determining the scroll method of a device.
//!
//! See the libinput documentation for details.

use serde::{Deserialize, Serialize};

/// The scroll method of a device.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ScrollMethod(pub u32);

/// Never send scroll events instead of pointer motion events.
pub const SCROLL_METHOD_NO_SCROLL: ScrollMethod = ScrollMethod(0);

/// Send scroll events when two fingers are logically down on the device.
pub const SCROLL_METHOD_TWO_FINGERS: ScrollMethod = ScrollMethod(1 << 0);

/// Send scroll events when a finger moves along the bottom or right edge of a device.
pub const SCROLL_METHOD_EDGE: ScrollMethod = ScrollMethod(1 << 1);

/// Send scroll events when a button is down and the device moves along a scroll-capable axis.
pub const SCROLL_METHOD_ON_BUTTON_DOWN: ScrollMethod = ScrollMethod(1 << 2);
