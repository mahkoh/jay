//! Tools for configuring the look of the compositor.

use serde::{Deserialize, Serialize};

/// A color.
///
/// When specifying RGBA values of a color, the RGB values can either be specified
/// *straight* or *premultiplied*. Premultiplied means that the RGB values have already
/// been multiplied by the alpha value.
///
/// Given a color, to reduce its opacity by half,
///
/// - if you're working with premultiplied values, you would multiply each component by `0.5`;
/// - if you're working with straight values, you would multiply only the alpha component by `0.5`.
///
/// When using hexadecimal notation, `#RRGGBBAA`, the RGB values are usually straight.
// values are stored premultiplied
#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

fn to_f32(c: u8) -> f32 {
    c as f32 / 255f32
}

fn to_u8(c: f32) -> u8 {
    (c * 255f32) as u8
}

fn validate_f32(f: f32) -> bool {
    f >= 0.0 && f <= 1.0
}

fn validate_f32_all(f: [f32; 4]) -> bool {
    if !f.into_iter().all(validate_f32) {
        log::warn!(
            "f32 values {:?} are not in the valid color range. Using solid black instead xyz",
            f
        );
        return false;
    }
    true
}

impl Color {
    /// Solid black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Creates a new color from `u8` RGB values.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: to_f32(r),
            g: to_f32(g),
            b: to_f32(b),
            a: 1.0,
        }
    }

    /// Creates a new color from straight `u8` RGBA values.
    pub fn new_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new_f32_straight(to_f32(r), to_f32(g), to_f32(b), to_f32(a))
    }

    /// Creates a new color from premultiplied `f32` RGBA values.
    pub fn new_f32_premultiplied(r: f32, g: f32, b: f32, a: f32) -> Self {
        if !validate_f32_all([r, g, b, a]) {
            Self::BLACK
        } else if r > a || g > a || b > a {
            log::warn!("f32 values {:?} are not valid valid for a premultiplied color. Using solid black instead.", [r, g, b, a]);
            Self::BLACK
        } else {
            Self { r, g, b, a }
        }
    }

    /// Creates a new color from straight `f32` RGBA values.
    pub fn new_f32_straight(r: f32, g: f32, b: f32, a: f32) -> Self {
        if !validate_f32_all([r, g, b, a]) {
            Self::BLACK
        } else {
            Self {
                r: r * a,
                g: g * a,
                b: b * a,
                a,
            }
        }
    }

    /// Creates a new color from `f32` RGB values.
    pub fn new_f32(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Converts the color to its premultiplied `f32` RGBA values.
    pub fn to_f32_premultiplied(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Converts the color to its straight `f32` RGBA values.
    pub fn to_f32_straight(&self) -> [f32; 4] {
        if self.a == 0.0 {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            let a = self.a;
            [self.r / a, self.g / a, self.b / a, a]
        }
    }

    /// Converts the color to its straight `u8` RGBA values.
    pub fn to_u8_straight(&self) -> [u8; 4] {
        let [r, g, b, a] = self.to_f32_straight();
        [to_u8(r), to_u8(g), to_u8(b), to_u8(a)]
    }
}

/// Resets all sizes to their defaults.
pub fn reset_sizes() {
    get!().reset_sizes();
}

/// Resets all colors to their defaults.
pub fn reset_colors() {
    get!().reset_colors();
}

/// Returns the current font.
pub fn get_font() -> String {
    get!().get_font()
}

/// Sets the font.
///
/// Default: `monospace 8`.
///
/// The font name should be specified in [pango][pango] syntax.
///
/// [pango]: https://docs.gtk.org/Pango/type_func.FontDescription.from_string.html
pub fn set_font(font: &str) {
    get!().set_font(font)
}

/// Resets the font to the default.
///
/// Currently the default is `monospace 8`.
pub fn reset_font() {
    get!().reset_font()
}

/// Elements of the compositor whose color can be changed.
pub mod colors {
    use {
        crate::theme::Color,
        serde::{Deserialize, Serialize},
    };

    /// An element of the GUI whose color can be changed.
    #[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
    pub struct Colorable(#[doc(hidden)] pub u32);

    impl Colorable {
        /// Sets the color to an RGB value.
        pub fn set(self, r: u8, g: u8, b: u8) {
            let color = Color::new(r, g, b);
            get!().set_color(self, color);
        }

        /// Sets the color to a `Color` that might contain an alpha component.
        pub fn set_color(self, color: Color) {
            get!().set_color(self, color);
        }

        /// Gets the current color.
        pub fn get(self) -> Color {
            get!(Color::BLACK).get_color(self)
        }
    }

    macro_rules! colors {
        ($($(#[$attr:meta])* const $n:expr => $name:ident,)*) => {
            $(
                $(#[$attr])*
                pub const $name: Colorable = Colorable($n);
            )*
        }
    }

    colors! {
        /// The title background color of an unfocused window.
        ///
        /// Default: `#222222`.
        const 01 => UNFOCUSED_TITLE_BACKGROUND_COLOR,
        /// The title background color of a focused window.
        ///
        /// Default: `#285577`.
        const 02 => FOCUSED_TITLE_BACKGROUND_COLOR,
        /// The title background color of an unfocused window that was the last focused
        /// window in its container.
        ///
        /// Default: `#5f676a`.
        const 03 => FOCUSED_INACTIVE_TITLE_BACKGROUND_COLOR,
        /// The background color of the desktop.
        ///
        /// Default: `#001019`.
        ///
        /// You can use an application such as [swaybg][swaybg] to further customize the background.
        ///
        /// [swaybg]: https://github.com/swaywm/swaybg
        const 04 => BACKGROUND_COLOR,
        /// The background color of the bar.
        ///
        /// Default: `#000000`.
        const 05 => BAR_BACKGROUND_COLOR,
        /// The color of the 1px separator below window titles.
        ///
        /// Default: `#333333`.
        const 06 => SEPARATOR_COLOR,
        /// The color of the border between windows.
        ///
        /// Default: `#3f474a`.
        const 07 => BORDER_COLOR,
        /// The title text color of an unfocused window.
        ///
        /// Default: `#888888`.
        const 08 => UNFOCUSED_TITLE_TEXT_COLOR,
        /// The title text color of a focused window.
        ///
        /// Default: `#ffffff`.
        const 09 => FOCUSED_TITLE_TEXT_COLOR,
        /// The title text color of an unfocused window that was the last focused
        /// window in its container.
        ///
        /// Default: `#ffffff`.
        const 10 => FOCUSED_INACTIVE_TITLE_TEXT_COLOR,
        /// The color of the status text in the bar.
        ///
        /// Default: `#ffffff`.
        const 11 => BAR_STATUS_TEXT_COLOR,
        /// The title background color of an unfocused window that might be captured.
        ///
        /// Default: `#220303`.
        const 12 => CAPTURED_UNFOCUSED_TITLE_BACKGROUND_COLOR,
        /// The title background color of a focused window that might be captured.
        ///
        /// Default: `#772831`.
        const 13 => CAPTURED_FOCUSED_TITLE_BACKGROUND_COLOR,
        /// The title background color of a window that has requested attention.
        ///
        /// Default: `#23092c`.
        const 14 => ATTENTION_REQUESTED_BACKGROUND_COLOR,
        /// Color used to highlight parts of the UI.
        ///
        /// Default: `#9d28c67f`.
        const 15 => HIGHLIGHT_COLOR,
    }

    /// Sets the color of GUI element.
    pub fn set_color(element: Colorable, color: Color) {
        get!().set_color(element, color);
    }

    /// Gets the color of GUI element.
    pub fn get_color(element: Colorable) -> Color {
        get!(Color::BLACK).get_color(element)
    }
}

/// Elements of the compositor whose size can be changed.
pub mod sized {
    use serde::{Deserialize, Serialize};

    /// An element of the GUI whose size can be changed.
    #[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
    pub struct Resizable(#[doc(hidden)] pub u32);

    impl Resizable {
        /// Gets the current size.
        pub fn get(self) -> i32 {
            get!(0).get_size(self)
        }

        /// Sets the size.
        pub fn set(self, size: i32) {
            get!().set_size(self, size)
        }
    }

    macro_rules! sizes {
        ($($(#[$attr:meta])* const $n:expr => $name:ident,)*) => {
            $(
                $(#[$attr])*
                pub const $name: Resizable = Resizable($n);
            )*
        }
    }

    sizes! {
        /// The height of window titles.
        ///
        /// Default: 17
        const 01 => TITLE_HEIGHT,
        /// The width of borders between windows.
        ///
        /// Default: 4
        const 02 => BORDER_WIDTH,
    }
}
