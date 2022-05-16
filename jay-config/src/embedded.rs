//! Tools to configure the compositor in embedded environments.

use crate::input::InputDevice;

/// Grab the input device.
///
/// This usually only works if the compositor is running as an application under X. It will
/// probably not work under XWayland.
pub fn grab_input_device(kb: InputDevice, grab: bool) {
    get!().grab(kb, grab);
}
