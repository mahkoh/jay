use {
    crate::utils::clonecell::CloneCell,
    std::{cell::Cell, cmp::Ordering, ops::Mul, sync::Arc},
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Eq for Color {}

impl Ord for Color {
    fn cmp(&self, other: &Self) -> Ordering {
        self.r
            .total_cmp(&other.r)
            .then_with(|| self.g.total_cmp(&other.g))
            .then_with(|| self.b.total_cmp(&other.b))
            .then_with(|| self.a.total_cmp(&other.a))
    }
}

impl Mul<f32> for Color {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
            a: self.a * rhs,
        }
    }
}

impl PartialOrd for Color {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn to_f32(c: u8) -> f32 {
    c as f32 / 255f32
}

fn to_u8(c: f32) -> u8 {
    (c * 255f32).round() as u8
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const SOLID_BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub fn from_gray(g: u8) -> Self {
        Self::from_rgb(g, g, g)
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: to_f32(r),
            g: to_f32(g),
            b: to_f32(b),
            a: 1.0,
        }
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: to_f32(r),
            g: to_f32(g),
            b: to_f32(b),
            a: to_f32(a),
        }
    }

    pub fn from_u32_rgba_premultiplied(r: u32, g: u32, b: u32, a: u32) -> Self {
        fn to_f32(c: u32) -> f32 {
            ((c as f64) / (u32::MAX as f64)) as f32
        }
        Self {
            r: to_f32(r),
            g: to_f32(g),
            b: to_f32(b),
            a: to_f32(a),
        }
    }

    pub fn from_rgba_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        let alpha = to_f32(a);
        Self {
            r: to_f32(r) * alpha,
            g: to_f32(g) * alpha,
            b: to_f32(b) * alpha,
            a: alpha,
        }
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn to_rgba_premultiplied(self) -> [u8; 4] {
        [to_u8(self.r), to_u8(self.g), to_u8(self.b), to_u8(self.a)]
    }

    pub fn to_array_srgb(self, alpha: Option<f32>) -> [f32; 4] {
        let a = alpha.unwrap_or(1.0);
        [self.r * a, self.g * a, self.b * a, self.a * a]
    }

    pub fn to_array_linear(self, alpha: Option<f32>) -> [f32; 4] {
        fn to_linear(srgb: f32) -> f32 {
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        }
        let a1 = if self.a == 0.0 { 1.0 } else { self.a };
        let a2 = self.a * alpha.unwrap_or(1.0);
        [
            to_linear(self.r / a1) * a2,
            to_linear(self.g / a1) * a2,
            to_linear(self.b / a1) * a2,
            a2,
        ]
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn and_then(self, other: &Color) -> Color {
        Color {
            r: self.r * (1.0 - other.a) + other.r,
            g: self.g * (1.0 - other.a) + other.g,
            b: self.b * (1.0 - other.a) + other.b,
            a: self.a * (1.0 - other.a) + other.a,
        }
    }
}

impl From<jay_config::theme::Color> for Color {
    fn from(f: jay_config::theme::Color) -> Self {
        let [r, g, b, a] = f.to_f32_premultiplied();
        Self { r, g, b, a }
    }
}

macro_rules! colors {
    ($($name:ident = $colors:tt,)*) => {
        pub struct ThemeColors {
            $(
                pub $name: Cell<Color>,
            )*
        }

        impl ThemeColors {
            pub fn reset(&self) {
                let default = Self::default();
                $(
                    self.$name.set(default.$name.get());
                )*
            }
        }

        impl Default for ThemeColors {
            fn default() -> Self {
                Self {
                    $(
                        $name: Cell::new(colors!(@colors $colors)),
                    )*
                }
            }
        }
    };
    (@colors ($r:expr, $g:expr, $b:expr)) => {
        Color::from_rgb($r, $g, $b)
    };
    (@colors ($r:expr, $g:expr, $b:expr, $a:expr)) => {
        Color::from_rgba_straight($r, $g, $b, $a)
    };
}

colors! {
    background = (0x00, 0x10, 0x19),
    unfocused_title_background = (0x22, 0x22, 0x22),
    focused_title_background = (0x28, 0x55, 0x77),
    captured_unfocused_title_background = (0x22, 0x03, 0x03),
    captured_focused_title_background = (0x77, 0x28, 0x31),
    focused_inactive_title_background = (0x5f, 0x67, 0x6a),
    unfocused_title_text = (0x88, 0x88, 0x88),
    focused_title_text = (0xff, 0xff, 0xff),
    focused_inactive_title_text = (0xff, 0xff, 0xff),
    separator = (0x33, 0x33, 0x33),
    border = (0x3f, 0x47, 0x4a),
    bar_background = (0x00, 0x00, 0x00),
    bar_text = (0xff, 0xff, 0xff),
    attention_requested_background = (0x23, 0x09, 0x2c),
    highlight = (0x9d, 0x28, 0xc6, 0x7f),
}

macro_rules! sizes {
    ($($name:ident = ($min:expr, $max:expr, $def:expr),)*) => {
        pub struct ThemeSizes {
            $(
                pub $name: Cell<i32>,
            )*
        }

        #[derive(Copy, Clone, Debug)]
        #[expect(non_camel_case_types)]
        pub enum ThemeSized {
            $(
                $name,
            )*
        }

        impl ThemeSized {
            pub fn min(self) -> i32 {
                match self {
                    $(
                        Self::$name => $min,
                    )*
                }
            }

            pub fn max(self) -> i32 {
                match self {
                    $(
                        Self::$name => $max,
                    )*
                }
            }

            pub fn field(self, theme: &Theme) -> &Cell<i32> {
                let sizes = &theme.sizes;
                match self {
                    $(
                        Self::$name => &sizes.$name,
                    )*
                }
            }

            pub fn name(self) -> &'static str {
                match self {
                    $(
                        Self::$name => stringify!($name),
                    )*
                }
            }
        }

        impl ThemeSizes {
            pub fn reset(&self) {
                let default = Self::default();
                $(
                    self.$name.set(default.$name.get());
                )*
            }
        }

        impl Default for ThemeSizes {
            fn default() -> Self {
                Self {
                    $(
                        $name: Cell::new($def),
                    )*
                }
            }
        }
    }
}

sizes! {
    title_height = (1, 1000, 17),
    border_width = (1, 1000, 4),
}

pub const DEFAULT_FONT: &str = "monospace 8";

pub struct Theme {
    pub colors: ThemeColors,
    pub sizes: ThemeSizes,
    pub font: CloneCell<Arc<String>>,
    pub default_font: Arc<String>,
}

impl Default for Theme {
    fn default() -> Self {
        let default_font = Arc::new(DEFAULT_FONT.to_string());
        Self {
            colors: Default::default(),
            sizes: Default::default(),
            font: CloneCell::new(default_font.clone()),
            default_font,
        }
    }
}
