//! Tools for configuring Xwayland.

use serde::{Deserialize, Serialize};

/// The scaling mode of X windows.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct XScalingMode(pub u32);

impl XScalingMode {
    /// The default mode.
    ///
    /// Currently this means that windows are rendered at the lowest scale and then
    /// upscaled if necessary.
    pub const DEFAULT: Self = Self(0);
    /// Windows are rendered at the highest integer scale and then downscaled.
    ///
    /// This has significant performance implications unless the window is running on the
    /// output with the highest scale and that scale is an integer scale.
    ///
    /// For example, on a 3840x2160 output with a 1.5 scale, a fullscreen window will be
    /// rendered at 3840x2160 * 2 / 1.5 = 5120x2880 pixels and then downscaled to
    /// 3840x2160. This overhead gets worse the lower the scale of the output is.
    ///
    /// Additionally, this mode requires the X window to scale its contents itself. In the
    /// example above, you might achieve this by setting the environment variable
    /// `GDK_SCALE=2`.
    pub const DOWNSCALED: Self = Self(1);
}

/// Sets the scaling mode for X windows.
pub fn set_x_scaling_mode(mode: XScalingMode) {
    get!().set_x_scaling_mode(mode)
}
