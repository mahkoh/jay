#![expect(clippy::excessive_precision)]

use {
    crate::{cmm::cmm_transfer_function::TransferFunction, utils::clonecell::CloneCell},
    num_traits::Float,
    std::{cell::Cell, cmp::Ordering, ops::Mul, sync::Arc},
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

    pub fn new(transfer_function: TransferFunction, mut r: f32, mut g: f32, mut b: f32) -> Self {
        fn srgb(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
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
        fn ext_srgb(c: f32) -> f32 {
            let c = c.clamp(-0.6038, 7.5913);
            if c <= -0.0031308 {
                -1.055 * (-c).powf(1.0 / 2.4) + 0.055
            } else if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }
        fn bt1886(c: f32) -> f32 {
            if c < 0.081 {
                c / 4.5
            } else {
                ((c + 0.099) / 1.099).powf(1.0 / 0.45)
            }
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
            c.powf(2.2)
        }
        fn gamma28(c: f32) -> f32 {
            c.powf(2.8)
        }
        macro_rules! convert {
            ($tf:ident) => {{
                r = $tf(r);
                g = $tf(g);
                b = $tf(b);
            }};
        }
        match transfer_function {
            TransferFunction::Srgb => convert!(srgb),
            TransferFunction::Linear => convert!(linear),
            TransferFunction::St2084Pq => convert!(st2084_pq),
            TransferFunction::Bt1886 => convert!(bt1886),
            TransferFunction::Gamma22 => convert!(gamma22),
            TransferFunction::Gamma28 => convert!(gamma28),
            TransferFunction::St240 => convert!(st240),
            TransferFunction::ExtSrgb => convert!(ext_srgb),
            TransferFunction::Log100 => convert!(log100),
            TransferFunction::Log316 => convert!(log316),
            TransferFunction::St428 => convert!(st428),
        }
        Self { r, g, b, a: 1.0 }
    }

    pub fn new_premultiplied(
        transfer_function: TransferFunction,
        mut r: f32,
        mut g: f32,
        mut b: f32,
        a: f32,
    ) -> Self {
        if transfer_function == TransferFunction::Linear {
            return Self { r, g, b, a };
        }
        if a < 1.0 && a > 0.0 {
            for c in [&mut r, &mut g, &mut b] {
                *c /= a;
            }
        }
        let mut c = Self::new(transfer_function, r, g, b);
        if a < 1.0 {
            c = c * a;
        }
        c
    }

    pub fn is_opaque(&self) -> bool {
        self.a >= 1.0
    }

    pub fn from_gray_srgb(g: u8) -> Self {
        Self::from_srgb(g, g, g)
    }

    pub fn from_srgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(TransferFunction::Srgb, to_f32(r), to_f32(g), to_f32(b))
    }

    pub fn from_srgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new_premultiplied(
            TransferFunction::Srgb,
            to_f32(r),
            to_f32(g),
            to_f32(b),
            to_f32(a),
        )
    }

    pub fn from_u32_premultiplied(
        transfer_function: TransferFunction,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    ) -> Self {
        fn to_f32(c: u32) -> f32 {
            ((c as f64) / (u32::MAX as f64)) as f32
        }
        Self::new_premultiplied(
            transfer_function,
            to_f32(r),
            to_f32(g),
            to_f32(b),
            to_f32(a),
        )
    }

    pub fn from_srgba_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        let mut c = Self::new(TransferFunction::Srgb, to_f32(r), to_f32(g), to_f32(b));
        if a < 255 {
            c = c * to_f32(a);
        }
        c
    }

    pub fn to_srgba_premultiplied(self) -> [u8; 4] {
        let [r, g, b, a] = self.to_array(TransferFunction::Srgb);
        [to_u8(r), to_u8(g), to_u8(b), to_u8(a)]
    }

    pub fn to_array(self, transfer_function: TransferFunction) -> [f32; 4] {
        self.to_array2(transfer_function, None)
    }

    pub fn to_array2(self, transfer_function: TransferFunction, alpha: Option<f32>) -> [f32; 4] {
        let mut res = [self.r, self.g, self.b, self.a];
        fn srgb(c: f32) -> f32 {
            if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }
        fn linear(c: f32) -> f32 {
            c
        }
        fn st2084_pq(c: f32) -> f32 {
            let c = c.clamp(0.0, 1.0);
            let num = 0.8359375 + 18.8515625 * c.powf(0.1593017578125);
            let den = 1.0 + 18.6875 * c.powf(0.1593017578125);
            (num / den).powf(78.84375)
        }
        fn ext_srgb(c: f32) -> f32 {
            if c < -0.04045 {
                -((c - 0.055) / -1.055).powf(2.4)
            } else if c < 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        fn bt1886(c: f32) -> f32 {
            if c < 0.018 {
                4.5 * c
            } else {
                1.099 * c.powf(0.45) - 0.099
            }
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
            c.powf(1.0 / 2.2)
        }
        fn gamma28(c: f32) -> f32 {
            c.powf(1.0 / 2.8)
        }
        macro_rules! convert {
            ($tf:ident) => {{
                for c in &mut res[..3] {
                    *c = $tf(*c);
                }
            }};
        }
        if transfer_function != TransferFunction::Linear {
            if self.a < 1.0 && self.a > 0.0 {
                for c in &mut res[..3] {
                    *c /= self.a;
                }
            }
            match transfer_function {
                TransferFunction::Srgb => convert!(srgb),
                TransferFunction::Linear => convert!(linear),
                TransferFunction::St2084Pq => convert!(st2084_pq),
                TransferFunction::Bt1886 => convert!(bt1886),
                TransferFunction::Gamma22 => convert!(gamma22),
                TransferFunction::Gamma28 => convert!(gamma28),
                TransferFunction::St240 => convert!(st240),
                TransferFunction::ExtSrgb => convert!(ext_srgb),
                TransferFunction::Log100 => convert!(log100),
                TransferFunction::Log316 => convert!(log316),
                TransferFunction::St428 => convert!(st428),
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
}

impl From<jay_config::theme::Color> for Color {
    fn from(f: jay_config::theme::Color) -> Self {
        let [r, g, b, a] = f.to_f32_premultiplied();
        Self::new_premultiplied(TransferFunction::Srgb, r, g, b, a)
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
