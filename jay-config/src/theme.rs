//! Tools for configuring the look of the compositor.

use crate::_private::WindowThemeKind;
use crate::theme::colors::Colorable;
use crate::theme::sized::Resizable;
use crate::window::Window;
use serde::Deserialize;
use serde::Serialize;
use std::ops::Deref;

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
            log::warn!(
                "f32 values {:?} are not valid for a premultiplied color. Using solid black instead.",
                [r, g, b, a]
            );
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
/// See also [`set_bar_font`] and [`set_title_font`].
///
/// The font name should be specified in [pango][pango] syntax.
///
/// [pango]: https://docs.gtk.org/Pango/type_func.FontDescription.from_string.html
pub fn set_font(font: &str) {
    get!().set_font(font)
}

/// Sets the font used by the bar.
///
/// If this function is not called, the font set by [`set_font`] is used. See that
/// function for more details.
pub fn set_bar_font(font: &str) {
    get!().set_bar_font(font)
}

/// Sets the font used by window titles.
///
/// If this function is not called, the font set by [`set_font`] is used. See that
/// function for more details.
pub fn set_title_font(font: &str) {
    get!().set_title_font(font)
}

/// Resets the fonts to the defaults.
///
/// Currently the default is `monospace 8`.
pub fn reset_font() {
    get!().reset_font()
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum BarPosition {
    #[default]
    Top,
    Bottom,
}

/// Sets the position of the bar.
///
/// Default: `Top`.
pub fn set_bar_position(position: BarPosition) {
    get!().set_bar_position(position);
}

/// Gets the position of the bar.
pub fn get_bar_position() -> BarPosition {
    get!(BarPosition::Top).get_bar_position()
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ContainerBorders {
    /// Only separators are drawn between children.
    #[default]
    Separators,
    /// A border is drawn around the entire container.
    Full,
    /// A border is drawn around the entire container, in addition to the separators
    /// between children, unless the container has only one child and is the root
    /// container of the workspace.
    FullSmart,
}

/// Sets the container border style.
///
/// Default: `Separators`.
pub fn set_container_borders(borders: ContainerBorders) {
    get!().set_container_borders(borders);
}

/// Gets the container border style.
pub fn get_container_borders() -> ContainerBorders {
    get!(ContainerBorders::Separators).get_container_borders()
}

/// Sets the proportional fonts used by egui windows.
///
/// The default is `["sans-serif", "Noto Sans", "Noto Color Emoji"]`.
pub fn set_egui_proportional_fonts<'a>(fonts: impl IntoIterator<Item = &'a str>) {
    get!().set_egui_fonts(Some(fonts.into_iter().collect()), None);
}

/// Sets the monospace fonts used by egui windows.
///
/// The default is `["monospace", "Noto Sans Mono", "Noto Color Emoji"]`.
pub fn set_egui_monospace_fonts<'a>(fonts: impl IntoIterator<Item = &'a str>) {
    get!().set_egui_fonts(None, Some(fonts.into_iter().collect()));
}

/// Sets whether window icons set by the client are shown.
///
/// The default is `true`.
pub fn set_show_window_icons(show: bool) {
    get!().set_show_window_icons(show);
}

/// Sets whether window icons set by the client are rendered as grayscale.
///
/// This is only supported on the Vulkan renderer.
///
/// The default is `false`.
pub fn set_window_icons_grayscale(grayscale: bool) {
    get!().set_window_icons_grayscale(grayscale);
}

/// Theme overrides of a window or container.
///
/// This type implements the functionality shared by [`WindowTheme`] and
/// [`ContainerTheme`]. Both dereference to this type. See their documentation for the
/// supported settings.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ThemeOverrides {
    pub(crate) window: Window,
    pub(crate) kind: WindowThemeKind,
}

impl ThemeOverrides {
    /// Returns the window that the overrides belong to.
    pub fn window(&self) -> Window {
        self.window
    }

    /// Removes all overrides.
    pub fn reset(&self) {
        get!().reset_window_theme(self.window, self.kind);
    }

    /// Sets the color of a GUI element.
    pub fn set_color(&self, element: Colorable, color: Color) {
        get!().set_window_theme_color(self.window, self.kind, element, Some(color));
    }

    /// Removes the color override of a GUI element.
    ///
    /// See also [`set_color`](Self::set_color).
    pub fn unset_color(&self, element: Colorable) {
        get!().set_window_theme_color(self.window, self.kind, element, None);
    }

    /// Gets the color override of a GUI element.
    pub fn get_color(&self, element: Colorable) -> Option<Color> {
        get!().get_window_theme_color(self.window, self.kind, element)
    }

    /// Sets the size of a GUI element.
    pub fn set_size(&self, element: Resizable, size: i32) {
        get!().set_window_theme_size(self.window, self.kind, element, Some(size));
    }

    /// Removes the size override of a GUI element.
    ///
    /// See also [`set_size`](Self::set_size).
    pub fn unset_size(&self, element: Resizable) {
        get!().set_window_theme_size(self.window, self.kind, element, None);
    }

    /// Gets the size override of a GUI element.
    pub fn get_size(&self, element: Resizable) -> Option<i32> {
        get!().get_window_theme_size(self.window, self.kind, element)
    }

    /// Sets whether titles are shown.
    ///
    /// See also [`set_show_titles`](crate::set_show_titles).
    pub fn set_show_titles(&self, show: bool) {
        get!().set_window_theme_show_titles(self.window, self.kind, Some(show));
    }

    /// Removes the override of whether titles are shown.
    ///
    /// See also [`set_show_titles`](Self::set_show_titles).
    pub fn unset_show_titles(&self) {
        get!().set_window_theme_show_titles(self.window, self.kind, None);
    }

    /// Gets the override of whether titles are shown.
    pub fn get_show_titles(&self) -> Option<bool> {
        get!().get_window_theme_show_titles(self.window, self.kind)
    }

    /// Sets whether window icons set by the client are shown.
    ///
    /// See also [`set_show_window_icons`].
    pub fn set_show_window_icons(&self, show: bool) {
        get!().set_window_theme_show_window_icons(self.window, self.kind, Some(show));
    }

    /// Removes the override of whether window icons set by the client are shown.
    ///
    /// See also [`set_show_window_icons`](Self::set_show_window_icons).
    pub fn unset_show_window_icons(&self) {
        get!().set_window_theme_show_window_icons(self.window, self.kind, None);
    }

    /// Sets whether window icons set by the client are rendered as grayscale.
    ///
    /// See also [`set_window_icons_grayscale`].
    pub fn set_window_icons_grayscale(&self, grayscale: bool) {
        get!().set_window_theme_window_icons_grayscale(self.window, self.kind, Some(grayscale));
    }

    /// Removes the override of whether window icons set by the client are rendered as
    /// grayscale.
    ///
    /// See also [`set_window_icons_grayscale`](Self::set_window_icons_grayscale).
    pub fn unset_window_icons_grayscale(&self) {
        get!().set_window_theme_window_icons_grayscale(self.window, self.kind, None);
    }

    /// Sets the font used by window titles.
    ///
    /// See also [`set_title_font`].
    pub fn set_title_font(&self, font: &str) {
        get!().set_window_theme_title_font(self.window, self.kind, Some(font));
    }

    /// Removes the override of the font used by window titles.
    ///
    /// See also [`set_title_font`](Self::set_title_font).
    pub fn unset_title_font(&self) {
        get!().set_window_theme_title_font(self.window, self.kind, None);
    }
}

/// Theme overrides of a window.
///
/// This object is returned by [`Window::theme`]. It contains the theme of the window
/// itself. How a container decorates its children is configured with
/// [`ContainerTheme`].
///
/// Settings that are not set fall back to the theme of the parent container, if any, and
/// then to the global theme. Not every setting has an effect on every window.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct WindowTheme(pub(crate) ThemeOverrides);

impl Deref for WindowTheme {
    type Target = ThemeOverrides;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

const _: () = {
    use colors::*;
    use sized::*;

    impl WindowTheme {
        /// Sets the color of a GUI element.
        ///
        /// Floating windows use the following elements. This theme takes priority over the
        /// global theme.
        ///
        /// - [`BORDER_COLOR`]
        /// - [`FOCUSED_BORDER_COLOR`]
        /// - [`SEPARATOR_COLOR`]
        /// - [`UNFOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`FOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`ATTENTION_REQUESTED_BACKGROUND_COLOR`]
        /// - [`UNFOCUSED_TITLE_TEXT_COLOR`]
        /// - [`FOCUSED_TITLE_TEXT_COLOR`]
        ///
        /// Tiled windows use the following elements. This theme takes priority over the
        /// [`ContainerTheme`] of the parent container, which takes priority over the global
        /// theme.
        ///
        /// - [`FOCUSED_BORDER_COLOR`]
        /// - [`BORDER_COLOR`]: only used as the default of `FOCUSED_BORDER_COLOR`. The
        ///   other borders use the border color of the container.
        /// - [`UNFOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`FOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`FOCUSED_INACTIVE_TITLE_BACKGROUND_COLOR`]
        /// - [`ATTENTION_REQUESTED_BACKGROUND_COLOR`]
        /// - [`UNFOCUSED_TITLE_TEXT_COLOR`]
        /// - [`FOCUSED_TITLE_TEXT_COLOR`]
        /// - [`FOCUSED_INACTIVE_TITLE_TEXT_COLOR`]
        ///
        /// For tiled windows, the separator color is a property of the container and is set
        /// with [`ContainerTheme::set_color`].
        pub fn set_color(&self, element: Colorable, color: Color) {
            self.0.set_color(element, color)
        }

        /// Sets the size of a GUI element.
        ///
        /// The following elements are supported:
        ///
        /// - [`TITLE_HEIGHT`]
        /// - [`BORDER_WIDTH`]
        ///
        /// They are only used for floating windows. This theme takes priority over the
        /// global theme. For tiled windows, these sizes are properties of the container and
        /// are set with [`ContainerTheme::set_size`].
        pub fn set_size(&self, element: Resizable, size: i32) {
            self.0.set_size(element, size)
        }

        /// Sets whether titles are shown.
        ///
        /// This is only used for floating windows. This theme takes priority over the global
        /// theme. For tiled windows, this is a property of the container and is set with
        /// [`ContainerTheme::set_show_titles`].
        ///
        /// See also [`set_show_titles`](crate::set_show_titles).
        pub fn set_show_titles(&self, show: bool) {
            self.0.set_show_titles(show)
        }

        /// Sets whether window icons set by the client are shown.
        ///
        /// For floating windows, this theme takes priority over the global theme. For tiled
        /// windows, this theme takes priority over the [`ContainerTheme`] of the parent
        /// container, which takes priority over the global theme.
        ///
        /// See also [`set_show_window_icons`].
        pub fn set_show_window_icons(&self, show: bool) {
            self.0.set_show_window_icons(show)
        }

        /// Sets whether window icons set by the client are rendered as grayscale.
        ///
        /// For floating windows, this theme takes priority over the global theme. For tiled
        /// windows, this theme takes priority over the [`ContainerTheme`] of the parent
        /// container, which takes priority over the global theme.
        ///
        /// See also [`set_window_icons_grayscale`].
        pub fn set_window_icons_grayscale(&self, grayscale: bool) {
            self.0.set_window_icons_grayscale(grayscale)
        }

        /// Sets the font used by window titles.
        ///
        /// For floating windows, this theme takes priority over the global theme. For tiled
        /// windows, this theme takes priority over the [`ContainerTheme`] of the parent
        /// container, which takes priority over the global theme.
        ///
        /// See also [`set_title_font`].
        pub fn set_title_font(&self, font: &str) {
            self.0.set_title_font(font)
        }
    }
};

/// Theme overrides of how a container decorates its children.
///
/// This object is returned by [`Window::container_theme`]. It has no effect if the
/// window is not a container.
///
/// Settings that are not set fall back to the global theme. Not every setting has an
/// effect on every container.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ContainerTheme(pub(crate) ThemeOverrides);

impl Deref for ContainerTheme {
    type Target = ThemeOverrides;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

const _: () = {
    use colors::*;
    use sized::*;

    impl ContainerTheme {
        /// Sets the color of a GUI element.
        ///
        /// The following elements apply to the container as a whole. This theme takes
        /// priority over the global theme.
        ///
        /// - [`BORDER_COLOR`]
        /// - [`SEPARATOR_COLOR`]
        ///
        /// The following elements apply to the decorations of each child. The
        /// [`WindowTheme`] of the child takes priority over this theme, which takes priority
        /// over the global theme.
        ///
        /// - [`FOCUSED_BORDER_COLOR`]
        /// - [`BORDER_COLOR`]: only used as the default of `FOCUSED_BORDER_COLOR`.
        /// - [`UNFOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`FOCUSED_TITLE_BACKGROUND_COLOR`]
        /// - [`FOCUSED_INACTIVE_TITLE_BACKGROUND_COLOR`]
        /// - [`ATTENTION_REQUESTED_BACKGROUND_COLOR`]
        /// - [`UNFOCUSED_TITLE_TEXT_COLOR`]
        /// - [`FOCUSED_TITLE_TEXT_COLOR`]
        /// - [`FOCUSED_INACTIVE_TITLE_TEXT_COLOR`]
        pub fn set_color(&self, element: Colorable, color: Color) {
            self.0.set_color(element, color)
        }

        /// Sets the size of a GUI element.
        ///
        /// The following elements are supported:
        ///
        /// - [`TITLE_HEIGHT`]
        /// - [`BORDER_WIDTH`]
        ///
        /// They apply to the container as a whole. This theme takes priority over the global
        /// theme.
        pub fn set_size(&self, element: Resizable, size: i32) {
            self.0.set_size(element, size)
        }

        /// Sets whether titles are shown.
        ///
        /// This applies to the container as a whole. This theme takes priority over the
        /// global theme.
        ///
        /// See also [`set_show_titles`](crate::set_show_titles).
        pub fn set_show_titles(&self, show: bool) {
            self.0.set_show_titles(show)
        }

        /// Sets whether window icons set by the client are shown.
        ///
        /// This applies to the decorations of each child. The [`WindowTheme`] of the child
        /// takes priority over this theme, which takes priority over the global theme.
        ///
        /// See also [`set_show_window_icons`].
        pub fn set_show_window_icons(&self, show: bool) {
            self.0.set_show_window_icons(show)
        }

        /// Sets whether window icons set by the client are rendered as grayscale.
        ///
        /// This applies to the decorations of each child. The [`WindowTheme`] of the child
        /// takes priority over this theme, which takes priority over the global theme.
        ///
        /// See also [`set_window_icons_grayscale`].
        pub fn set_window_icons_grayscale(&self, grayscale: bool) {
            self.0.set_window_icons_grayscale(grayscale)
        }

        /// Sets the font used by window titles.
        ///
        /// This applies to the decorations of each child. The [`WindowTheme`] of the child
        /// takes priority over this theme, which takes priority over the global theme.
        ///
        /// See also [`set_title_font`].
        pub fn set_title_font(&self, font: &str) {
            self.0.set_title_font(font)
        }

        /// Sets the container border style.
        ///
        /// This theme takes priority over the global theme.
        ///
        /// See also [`set_container_borders`].
        pub fn set_container_borders(&self, borders: ContainerBorders) {
            get!().set_window_theme_container_borders(self.0.window, self.0.kind, Some(borders));
        }

        /// Removes the override of the container border style.
        ///
        /// See also [`set_container_borders`](Self::set_container_borders).
        pub fn unset_container_borders(&self) {
            get!().set_window_theme_container_borders(self.0.window, self.0.kind, None);
        }

        /// Gets the override of the container border style.
        pub fn get_container_borders(&self) -> Option<ContainerBorders> {
            get!(None).get_window_theme_container_borders(self.0.window, self.0.kind)
        }
    }
};

/// Elements of the compositor whose color can be changed.
pub mod colors {
    #![allow(unused_imports)]
    use crate::theme::Color;
    use crate::theme::ContainerBorders;
    use serde::Deserialize;
    use serde::Serialize;

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
        /// The color of the border between windows where at least one of the windows is
        /// focused.
        ///
        /// For containers, this requires `Full` [`ContainerBorders`].
        ///
        /// Default: The `BORDER` color.
        const 16 => FOCUSED_BORDER_COLOR,
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
    use serde::Deserialize;
    use serde::Serialize;

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
        /// The height of the bar.
        ///
        /// Defaults to the TITLE_HEIGHT if not set explicitly.
        ///
        /// Default: 17
        const 03 => BAR_HEIGHT,
        /// The width of the bar's separator.
        ///
        /// Default: 1
        const 04 => BAR_SEPARATOR_WIDTH,
    }
}
