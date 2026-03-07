#![expect(clippy::excessive_precision)]

use {
    crate::{
        cmm::cmm_eotf::{Eotf, bt1886_eotf_args, bt1886_inv_eotf_args},
        gfx_api::AlphaMode,
        utils::{clonecell::CloneCell, static_text::StaticText},
    },
    jay_config::theme::BarPosition as ConfigBarPosition,
    linearize::Linearize,
    num_traits::Float,
    std::{
        cell::Cell,
        cmp::Ordering,
        ops::{Add, Div, Mul},
        sync::Arc,
    },
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
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

    pub fn new(
        eotf: Eotf,
        alpha_mode: AlphaMode,
        mut r: f32,
        mut g: f32,
        mut b: f32,
        a: f32,
    ) -> Self {
        if eotf == Eotf::Linear {
            if alpha_mode == AlphaMode::Straight && a < 1.0 {
                for c in [&mut r, &mut g, &mut b] {
                    *c *= a;
                }
            }
            return Self { r, g, b, a };
        }
        if alpha_mode == AlphaMode::PremultipliedElectrical && a < 1.0 && a > 0.0 {
            for c in [&mut r, &mut g, &mut b] {
                *c /= a;
            }
        }
        #[inline(always)]
        fn linear(c: f32) -> f32 {
            c
        }
        fn st2084_pq(c: f32) -> f32 {
            let cp = c.powf(1.0 / 78.84375);
            let num = (cp - 0.8359375).max(0.0);
            let den = 18.8515625 - 18.6875 * cp;
            (num / den).powf(1.0 / 0.1593017578125)
        }
        fn st240(c: f32) -> f32 {
            if c < 0.0913 {
                c / 4.0
            } else {
                ((c + 0.1115) / 1.1115).powf(1.0 / 0.45)
            }
        }
        fn log100(c: f32) -> f32 {
            10.0.powf(2.0 * (c - 1.0))
        }
        fn log316(c: f32) -> f32 {
            10.0.powf(2.5 * (c - 1.0))
        }
        fn st428(c: f32) -> f32 {
            c.powf(2.6) * 52.37 / 48.0
        }
        fn gamma22(c: f32) -> f32 {
            c.signum() * c.abs().powf(2.2)
        }
        fn gamma24(c: f32) -> f32 {
            c.signum() * c.abs().powf(2.4)
        }
        fn gamma28(c: f32) -> f32 {
            c.signum() * c.abs().powf(2.8)
        }
        fn compound_power_2_4(c: f32) -> f32 {
            if c < 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        macro_rules! convert {
            ($tf:ident) => {{
                r = $tf(r);
                g = $tf(g);
                b = $tf(b);
            }};
        }
        match eotf {
            Eotf::Linear => convert!(linear),
            Eotf::St2084Pq => convert!(st2084_pq),
            Eotf::Bt1886(c) => {
                let [a1, a2, a3, a4] = bt1886_eotf_args(c);
                let bt1886 = |c: f32| -> f32 { a1 * ((a2 * c + a3).powf(2.4) - a4) };
                convert!(bt1886)
            }
            Eotf::Gamma22 => convert!(gamma22),
            Eotf::Gamma24 => convert!(gamma24),
            Eotf::Gamma28 => convert!(gamma28),
            Eotf::St240 => convert!(st240),
            Eotf::Log100 => convert!(log100),
            Eotf::Log316 => convert!(log316),
            Eotf::St428 => convert!(st428),
            Eotf::Pow(n) => {
                let e = n.eotf_f32();
                let pow = |c: f32| -> f32 { c.signum() * c.abs().powf(e) };
                convert!(pow)
            }
            Eotf::CompoundPower24 => convert!(compound_power_2_4),
        }
        if alpha_mode != AlphaMode::PremultipliedOptical && a < 1.0 {
            for c in [&mut r, &mut g, &mut b] {
                *c *= a;
            }
        }
        Self { r, g, b, a }
    }

    pub fn is_opaque(&self) -> bool {
        self.a >= 1.0
    }

    pub fn from_gray_srgb(g: u8) -> Self {
        Self::from_srgb(g, g, g)
    }

    pub fn from_srgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(
            Eotf::Gamma22,
            AlphaMode::PremultipliedOptical,
            to_f32(r),
            to_f32(g),
            to_f32(b),
            1.0,
        )
    }

    pub fn from_srgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            Eotf::Gamma22,
            AlphaMode::PremultipliedElectrical,
            to_f32(r),
            to_f32(g),
            to_f32(b),
            to_f32(a),
        )
    }

    pub fn from_u32(eotf: Eotf, alpha_mode: AlphaMode, r: u32, g: u32, b: u32, a: u32) -> Self {
        fn to_f32(c: u32) -> f32 {
            ((c as f64) / (u32::MAX as f64)) as f32
        }
        Self::new(eotf, alpha_mode, to_f32(r), to_f32(g), to_f32(b), to_f32(a))
    }

    pub fn from_srgba_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            Eotf::Gamma22,
            AlphaMode::Straight,
            to_f32(r),
            to_f32(g),
            to_f32(b),
            to_f32(a),
        )
    }

    pub fn to_srgba_premultiplied(self) -> [u8; 4] {
        let [r, g, b, a] = self.to_array(Eotf::Gamma22);
        [to_u8(r), to_u8(g), to_u8(b), to_u8(a)]
    }

    pub fn to_array(self, eotf: Eotf) -> [f32; 4] {
        self.to_array2(eotf, None)
    }

    pub fn to_array2(self, eotf: Eotf, alpha: Option<f32>) -> [f32; 4] {
        let mut res = [self.r, self.g, self.b, self.a];
        fn linear(c: f32) -> f32 {
            c
        }
        fn st2084_pq(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            let num = 0.8359375 + 18.8515625 * c.powf(0.1593017578125);
            let den = 1.0 + 18.6875 * c.powf(0.1593017578125);
            (num / den).powf(78.84375)
        }
        fn st240(c: f32) -> f32 {
            if c < 0.0228 {
                4.0 * c
            } else {
                1.1115 * c.powf(0.45) - 0.1115
            }
        }
        fn log100(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            if c < 0.01 { 0.0 } else { 1.0 + c.log10() / 2.0 }
        }
        fn log316(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            if c < 10.0.sqrt() / 1000.0 {
                0.0
            } else {
                1.0 + c.log10() / 2.5
            }
        }
        fn st428(c: f32) -> f32 {
            (48.0 * c / 52.37).powf(1.0 / 2.6)
        }
        fn gamma22(c: f32) -> f32 {
            c.signum() * c.abs().powf(1.0 / 2.2)
        }
        fn gamma24(c: f32) -> f32 {
            c.signum() * c.abs().powf(1.0 / 2.4)
        }
        fn gamma28(c: f32) -> f32 {
            c.signum() * c.abs().powf(1.0 / 2.8)
        }
        fn compound_power_2_4(c: f32) -> f32 {
            if c < 0.0031308 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }
        macro_rules! convert {
            ($tf:ident) => {{
                for c in &mut res[..3] {
                    *c = $tf(*c);
                }
            }};
        }
        if eotf != Eotf::Linear {
            if self.a < 1.0 && self.a > 0.0 {
                for c in &mut res[..3] {
                    *c /= self.a;
                }
            }
            match eotf {
                Eotf::Linear => convert!(linear),
                Eotf::St2084Pq => convert!(st2084_pq),
                Eotf::Bt1886(c) => {
                    let [a1, a2, a3, a4] = bt1886_inv_eotf_args(c);
                    let bt1886 = |c: f32| -> f32 { a1 * ((a2 * c + a3).powf(1.0 / 2.4) - a4) };
                    convert!(bt1886)
                }
                Eotf::Gamma22 => convert!(gamma22),
                Eotf::Gamma24 => convert!(gamma24),
                Eotf::Gamma28 => convert!(gamma28),
                Eotf::St240 => convert!(st240),
                Eotf::Log100 => convert!(log100),
                Eotf::Log316 => convert!(log316),
                Eotf::St428 => convert!(st428),
                Eotf::Pow(n) => {
                    let e = n.inv_eotf_f32();
                    let pow = |c: f32| -> f32 { c.signum() * c.abs().powf(e) };
                    convert!(pow)
                }
                Eotf::CompoundPower24 => convert!(compound_power_2_4),
            }
            if self.a < 1.0 {
                for c in &mut res[..3] {
                    *c *= self.a;
                }
            }
        }
        if let Some(a) = alpha {
            for c in &mut res {
                *c *= a;
            }
        }
        res
    }

    pub fn and_then(self, other: &Color) -> Color {
        Color {
            r: self.r * (1.0 - other.a) + other.r,
            g: self.g * (1.0 - other.a) + other.g,
            b: self.b * (1.0 - other.a) + other.b,
            a: self.a * (1.0 - other.a) + other.a,
        }
    }

    pub fn srgb_to_oklab(self) -> Oklab {
        if self.a == 0.0 {
            return Oklab {
                l: 0.0,
                a: 0.0,
                b: 0.0,
            };
        }

        let [r, g, b, _] = self.to_array2(Eotf::Linear, Some(1.0 / self.a));

        let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
        let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
        let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        let l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
        let a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
        let b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

        Oklab { l, a, b }
    }
}

impl From<jay_config::theme::Color> for Color {
    fn from(f: jay_config::theme::Color) -> Self {
        let [r, g, b, a] = f.to_f32_premultiplied();
        Self::new(
            Eotf::Gamma22,
            AlphaMode::PremultipliedElectrical,
            r,
            g,
            b,
            a,
        )
    }
}

macro_rules! colors {
    ($($name:ident = $colors:tt,)*) => {
        pub struct ThemeColors {
            $(
                pub $name: Cell<Color>,
            )*
        }

        #[derive(Copy, Clone, Debug, Linearize)]
        #[expect(non_camel_case_types)]
        pub enum ThemeColor {
            $(
                $name,
            )*
        }

        impl ThemeColor {
            pub fn field(self, theme: &Theme) -> &Cell<Color> {
                let colors = &theme.colors;
                match self {
                    $(
                        Self::$name => &colors.$name,
                    )*
                }
            }
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
        Color::from_srgb($r, $g, $b)
    };
    (@colors ($r:expr, $g:expr, $b:expr, $a:expr)) => {
        Color::from_srgba_straight($r, $g, $b, $a)
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

impl StaticText for ThemeColor {
    fn text(&self) -> &'static str {
        match self {
            ThemeColor::background => "Background",
            ThemeColor::unfocused_title_background => "Title Background (unfocused)",
            ThemeColor::focused_title_background => "Title Background (focused)",
            ThemeColor::captured_unfocused_title_background => {
                "Title Background (unfocused, captured)"
            }
            ThemeColor::captured_focused_title_background => "Title Background (focused, captured)",
            ThemeColor::focused_inactive_title_background => "Title Background (focused, inactive)",
            ThemeColor::unfocused_title_text => "Title Text (unfocused)",
            ThemeColor::focused_title_text => "Title Text (focused)",
            ThemeColor::focused_inactive_title_text => "Title Text (focused, inactive)",
            ThemeColor::separator => "Separator",
            ThemeColor::border => "Border",
            ThemeColor::bar_background => "Bar Background",
            ThemeColor::bar_text => "Bar Text",
            ThemeColor::attention_requested_background => "Attention Requested",
            ThemeColor::highlight => "Highlight",
        }
    }
}

pub struct ThemeSize {
    pub val: Cell<i32>,
    pub set: Cell<bool>,
}

impl ThemeSize {
    pub fn get(&self) -> i32 {
        self.val.get()
    }
}

macro_rules! sizes {
    ($($name:ident = ($min:expr, $max:expr, $def:expr),)*) => {
        pub struct ThemeSizes {
            $(
                pub $name: ThemeSize,
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

            pub fn field(self, theme: &Theme) -> &ThemeSize {
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
                    self.$name.val.set(default.$name.val.get());
                    self.$name.set.set(false);
                )*
            }
        }

        impl Default for ThemeSizes {
            fn default() -> Self {
                Self {
                    $(
                        $name: ThemeSize {
                            val: Cell::new($def),
                            set: Cell::new(false),
                        },
                    )*
                }
            }
        }
    }
}

impl ThemeSizes {
    pub fn bar_height(&self) -> i32 {
        if self.bar_height.set.get() {
            self.bar_height.val.get()
        } else {
            self.title_height.val.get()
        }
    }

    pub fn bar_separator_width(&self) -> i32 {
        self.bar_separator_width.get()
    }
}

sizes! {
    title_height = (0, 1000, 17),
    bar_height = (0, 1000, 17),
    border_width = (0, 1000, 4),
    bar_separator_width = (0, 1000, 1),
}

impl StaticText for ThemeSized {
    fn text(&self) -> &'static str {
        match self {
            ThemeSized::title_height => "Title Height",
            ThemeSized::bar_height => "Bar Height",
            ThemeSized::border_width => "Border Width",
            ThemeSized::bar_separator_width => "Bar Separator Width",
        }
    }
}

pub const DEFAULT_FONT: &str = "monospace 8";

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default, Linearize)]
pub enum BarPosition {
    #[default]
    Top,
    Bottom,
}

impl StaticText for BarPosition {
    fn text(&self) -> &'static str {
        match self {
            BarPosition::Top => "Top",
            BarPosition::Bottom => "Bottom",
        }
    }
}

impl TryFrom<ConfigBarPosition> for BarPosition {
    type Error = ();

    fn try_from(value: ConfigBarPosition) -> Result<Self, Self::Error> {
        let v = match value {
            ConfigBarPosition::Top => Self::Top,
            ConfigBarPosition::Bottom => Self::Bottom,
            _ => return Err(()),
        };
        Ok(v)
    }
}

impl Into<ConfigBarPosition> for BarPosition {
    fn into(self) -> ConfigBarPosition {
        match self {
            BarPosition::Top => ConfigBarPosition::Top,
            BarPosition::Bottom => ConfigBarPosition::Bottom,
        }
    }
}

pub struct Theme {
    pub colors: ThemeColors,
    pub sizes: ThemeSizes,
    pub font: CloneCell<Arc<String>>,
    pub bar_font: CloneCell<Option<Arc<String>>>,
    pub title_font: CloneCell<Option<Arc<String>>>,
    pub default_font: Arc<String>,
    pub show_titles: Cell<bool>,
    pub bar_position: Cell<BarPosition>,
}

impl Default for Theme {
    fn default() -> Self {
        let default_font = Arc::new(DEFAULT_FONT.to_string());
        Self {
            colors: Default::default(),
            sizes: Default::default(),
            font: CloneCell::new(default_font.clone()),
            bar_font: Default::default(),
            title_font: Default::default(),
            default_font,
            show_titles: Cell::new(true),
            bar_position: Default::default(),
        }
    }
}

impl Theme {
    pub fn title_font(&self) -> Arc<String> {
        self.title_font.get().unwrap_or_else(|| self.font.get())
    }

    pub fn bar_font(&self) -> Arc<String> {
        self.bar_font.get().unwrap_or_else(|| self.font.get())
    }

    pub fn title_height(&self) -> i32 {
        if self.show_titles.get() {
            self.sizes.title_height.get()
        } else {
            0
        }
    }

    pub fn title_underline_height(&self) -> i32 {
        if self.show_titles.get() { 1 } else { 0 }
    }

    pub fn title_plus_underline_height(&self) -> i32 {
        if self.show_titles.get() {
            self.sizes.title_height.get() + 1
        } else {
            0
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Oklab {
    pub fn to_srgb(self) -> Color {
        let l_ = self.l + 0.3963377774 * self.a + 0.2158037573 * self.b;
        let m_ = self.l - 0.1055613458 * self.a - 0.0638541728 * self.b;
        let s_ = self.l - 0.0894841775 * self.a - 1.2914855480 * self.b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

        Color::new(
            Eotf::Linear,
            AlphaMode::PremultipliedElectrical,
            r,
            g,
            b,
            1.0,
        )
    }

    pub fn to_oklch(self) -> Oklch {
        let c = (self.a * self.a + self.b * self.b).sqrt();
        let h = self.b.atan2(self.a);

        Oklch { l: self.l, c, h }
    }
}

impl Oklch {
    pub fn to_oklab(self) -> Oklab {
        let a = self.c * self.h.cos();
        let b = self.c * self.h.sin();

        Oklab { l: self.l, a, b }
    }
}

impl Add for Oklab {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            l: self.l + rhs.l,
            a: self.a + rhs.a,
            b: self.b + rhs.b,
        }
    }
}

impl Mul<f32> for Oklab {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            l: self.l * rhs,
            a: self.a * rhs,
            b: self.b * rhs,
        }
    }
}

impl Div<f32> for Oklab {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            l: self.l / rhs,
            a: self.a / rhs,
            b: self.b / rhs,
        }
    }
}
