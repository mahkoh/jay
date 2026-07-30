#![allow(clippy::excessive_precision)]

use crate::cmm::cmm_eotf::Eotf;
use crate::control_center::CCI_LOOK_AND_FEEL;
use crate::gfx_api::AlphaMode;
use crate::state::State;
use crate::tree::ContainerNode;
use crate::tree::FloatNode;
use crate::tree::NodeBase;
use crate::tree::NodeVisitorBase;
use crate::tree::OutputNode;
use crate::tree::SplitView;
use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::clonecell::CloneCell;
use crate::utils::static_text::StaticText;
use jay_algorithms::tf::eotfs;
use jay_algorithms::tf::inv_eotfs;
use jay_config::theme::BarPosition as ConfigBarPosition;
use jay_config::theme::ContainerBorders as ConfigContainerBorders;
use jay_proc::jay_clone;
use linearize::Linearize;
use linearize::StaticMap;
use std::cell::Cell;
use std::cmp::Ordering;
use std::ops::Add;
use std::ops::Div;
use std::ops::Mul;
use std::rc::Rc;
use std::sync::Arc;

#[jay_clone(Copy)]
#[derive(Debug, PartialEq)]
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
        macro_rules! convert2 {
            ($tf:path) => {{
                r = $tf(r);
                g = $tf(g);
                b = $tf(b);
            }};
        }
        macro_rules! convert {
            ($tf:ident) => {
                convert2!(eotfs::$tf::<()>)
            };
        }
        match eotf {
            Eotf::Linear => convert!(linear),
            Eotf::St2084Pq => convert!(st2084_pq),
            Eotf::Bt1886(c) => {
                let bt1886 = eotfs::bt1886::<()>(c.0);
                convert2!(bt1886)
            }
            Eotf::Gamma22 => convert!(gamma22),
            Eotf::Gamma24 => convert!(gamma24),
            Eotf::Gamma28 => convert!(gamma28),
            Eotf::St240 => convert!(st240),
            Eotf::Log100 => convert!(log100),
            Eotf::Log316 => convert!(log316),
            Eotf::St428 => convert!(st428),
            Eotf::Pow(n) => {
                let pow = eotfs::pow::<()>(n.eotf_f32());
                convert2!(pow)
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
        macro_rules! convert2 {
            ($tf:path) => {{
                for c in &mut res[..3] {
                    *c = $tf(*c);
                }
            }};
        }
        macro_rules! convert {
            ($tf:ident) => {
                convert2!(inv_eotfs::$tf::<()>)
            };
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
                    let bt1886 = inv_eotfs::bt1886::<()>(c.0);
                    convert2!(bt1886);
                }
                Eotf::Gamma22 => convert!(gamma22),
                Eotf::Gamma24 => convert!(gamma24),
                Eotf::Gamma28 => convert!(gamma28),
                Eotf::St240 => convert!(st240),
                Eotf::Log100 => convert!(log100),
                Eotf::Log316 => convert!(log316),
                Eotf::St428 => convert!(st428),
                Eotf::Pow(n) => {
                    let pow = inv_eotfs::pow::<()>(n.eotf_f32());
                    convert2!(pow);
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

    pub fn to_grayscale(mut self) -> Self {
        let y = 0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b;
        self.r = y;
        self.g = y;
        self.b = y;
        self
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

pub struct ThemeColor {
    pub val: Cell<Color>,
    pub set: Cell<bool>,
}

impl ThemeColor {
    pub fn get(&self) -> Color {
        self.val.get()
    }
}

macro_rules! colors {
    ($($name:ident = $colors:tt,)*) => {
        pub struct ThemeColors {
            $(
                pub $name: ThemeColor,
            )*
        }

        #[derive(Copy, Clone, Debug, Linearize, PartialEq)]
        #[allow(non_camel_case_types)]
        pub enum ThemeColored {
            $(
                $name,
            )*
        }

        impl ThemeColored {
            pub fn field(self, theme: &Theme) -> &ThemeColor {
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
                    self.$name.val.set(default.$name.get());
                    self.$name.set.set(false);
                )*
            }
        }

        impl Default for ThemeColors {
            fn default() -> Self {
                Self {
                    $(
                        $name: ThemeColor {
                            val: Cell::new(colors!(@colors $colors)),
                            set: Default::default(),
                        },
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
    focused_border = (0x3f, 0x47, 0x4a),
    bar_background = (0x00, 0x00, 0x00),
    bar_text = (0xff, 0xff, 0xff),
    attention_requested_background = (0x23, 0x09, 0x2c),
    highlight = (0x9d, 0x28, 0xc6, 0x7f),
}

impl StaticText for ThemeColored {
    fn text(&self) -> &'static str {
        match self {
            ThemeColored::background => "Background",
            ThemeColored::unfocused_title_background => "Title Background (unfocused)",
            ThemeColored::focused_title_background => "Title Background (focused)",
            ThemeColored::captured_unfocused_title_background => {
                "Title Background (unfocused, captured)"
            }
            ThemeColored::captured_focused_title_background => {
                "Title Background (focused, captured)"
            }
            ThemeColored::focused_inactive_title_background => {
                "Title Background (focused, inactive)"
            }
            ThemeColored::unfocused_title_text => "Title Text (unfocused)",
            ThemeColored::focused_title_text => "Title Text (focused)",
            ThemeColored::focused_inactive_title_text => "Title Text (focused, inactive)",
            ThemeColored::separator => "Separator",
            ThemeColored::border => "Border",
            ThemeColored::focused_border => "Focused Border",
            ThemeColored::bar_background => "Bar Background",
            ThemeColored::bar_text => "Bar Text",
            ThemeColored::attention_requested_background => "Attention Requested",
            ThemeColored::highlight => "Highlight",
        }
    }
}

pub struct ThemeSize {
    pub val: SplitView<Cell<i32>>,
    pub set: SplitView<Cell<bool>>,
}

impl ThemeSize {
    pub fn get(&self, tl: TreeTimeline) -> i32 {
        self.val[tl].get()
    }
}

macro_rules! sizes {
    ($($name:ident = ($min:expr, $max:expr, $def:expr),)*) => {
        pub struct ThemeSizes {
            $(
                pub $name: ThemeSize,
            )*
        }

        #[derive(Copy, Clone, Debug, Linearize)]
        #[allow(non_camel_case_types)]
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
            pub fn reset(&self, tl: TreeTimeline) {
                let default = Self::default();
                $(
                    self.$name.val[tl].set(default.$name.val[tl].get());
                    self.$name.set[tl].set(false);
                )*
            }
        }

        impl Default for ThemeSizes {
            fn default() -> Self {
                Self {
                    $(
                        $name: ThemeSize {
                            val: SplitView::from_fn(|_| Cell::new($def)),
                            set: Default::default(),
                        },
                    )*
                }
            }
        }
    }
}

impl ThemeSizes {
    pub fn bar_height(&self, tl: TreeTimeline) -> i32 {
        if self.bar_height.set[tl].get() {
            self.bar_height.val[tl].get()
        } else {
            self.title_height.val[tl].get()
        }
    }

    pub fn bar_separator_width(&self, tl: TreeTimeline) -> i32 {
        self.bar_separator_width.get(tl)
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

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default, Linearize)]
pub enum ContainerBordersSetting {
    #[default]
    Separators,
    Full,
    FullSmart,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContainerBorders {
    Separators,
    Full,
}

impl StaticText for ContainerBordersSetting {
    fn text(&self) -> &'static str {
        match self {
            ContainerBordersSetting::Separators => "Separators",
            ContainerBordersSetting::Full => "Full",
            ContainerBordersSetting::FullSmart => "FullSmart",
        }
    }
}

impl TryFrom<ConfigContainerBorders> for ContainerBordersSetting {
    type Error = ();

    fn try_from(value: ConfigContainerBorders) -> Result<Self, Self::Error> {
        let v = match value {
            ConfigContainerBorders::Separators => ContainerBordersSetting::Separators,
            ConfigContainerBorders::Full => ContainerBordersSetting::Full,
            ConfigContainerBorders::FullSmart => ContainerBordersSetting::FullSmart,
            _ => return Err(()),
        };
        Ok(v)
    }
}

impl Into<ConfigContainerBorders> for ContainerBordersSetting {
    fn into(self) -> ConfigContainerBorders {
        match self {
            ContainerBordersSetting::Separators => ConfigContainerBorders::Separators,
            ContainerBordersSetting::Full => ConfigContainerBorders::Full,
            ContainerBordersSetting::FullSmart => ConfigContainerBorders::FullSmart,
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
    pub show_titles: SplitView<Cell<bool>>,
    pub bar_position: SplitView<Cell<BarPosition>>,
    pub show_window_icons: Cell<bool>,
    pub window_icons_grayscale: Cell<bool>,
    pub container_borders: SplitView<Cell<ContainerBordersSetting>>,
    /// An empty set of overrides used by views without per-window overrides.
    empty_overrides: ThemeOverrides,
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
            show_titles: SplitView::from_fn(|_| Cell::new(true)),
            bar_position: Default::default(),
            show_window_icons: Cell::new(true),
            window_icons_grayscale: Cell::new(false),
            container_borders: Default::default(),
            empty_overrides: Default::default(),
        }
    }
}

impl Theme {
    /// Returns a view of this theme without any per-window overrides.
    pub fn view(&self) -> ThemeView<'_> {
        ThemeView::new(self, &self.empty_overrides)
    }

    /// Returns a view of this theme through `overrides`.
    pub fn view_with_overrides<'a>(
        &'a self,
        overrides: Option<&'a ThemeOverrides>,
    ) -> ThemeView<'a> {
        ThemeView::new(self, overrides.unwrap_or(&self.empty_overrides))
    }

    pub fn title_font(&self) -> Arc<String> {
        self.title_font.get().unwrap_or_else(|| self.font.get())
    }

    pub fn bar_font(&self) -> Arc<String> {
        self.bar_font.get().unwrap_or_else(|| self.font.get())
    }

    pub fn title_height(&self, tl: TreeTimeline) -> i32 {
        self.view().title_height(tl)
    }

    pub fn title_icon_size(&self, tl: TreeTimeline) -> i32 {
        self.view().title_icon_size(tl)
    }

    pub fn title_underline_height(&self, tl: TreeTimeline) -> i32 {
        self.view().title_underline_height(tl)
    }

    pub fn title_plus_underline_height(&self, tl: TreeTimeline) -> i32 {
        self.view().title_plus_underline_height(tl)
    }

    pub fn focused_border_color(&self, tl: TreeTimeline) -> Color {
        self.view().focused_border_color(tl)
    }
}

/// A sparse set of theme properties that override the global theme.
///
/// This is the generic mechanism used to make arbitrary theme properties settable on a
/// per-window basis. Adding a new overridable property requires no changes here: it is
/// enough to add it to the `sizes!`/`colors!` macro invocations above.
///
/// Note that overrides only affect the parts of the tree that are owned by a single
/// window. Sizes therefore only take effect for floating windows since the borders and
/// title bars of tiled windows are shared between siblings.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct ThemeOverrides {
    pub sizes: StaticMap<ThemeSized, SplitView<Cell<Option<i32>>>>,
    pub colors: StaticMap<ThemeColored, SplitView<Cell<Option<Color>>>>,
}

impl ThemeOverrides {
    pub fn is_empty(&self, tl: TreeTimeline) -> bool {
        self.sizes.values().all(|v| v[tl].get().is_none())
            && self.colors.values().all(|v| v[tl].get().is_none())
    }
}

/// A view of the theme through a set of overrides.
///
/// Views without per-window overrides borrow an empty set of overrides from the theme
/// itself.
///
/// All code that renders or lays out a window should retrieve theme properties through a
/// view instead of accessing [`Theme`] directly. That way per-window overrides apply
/// automatically to every property.
#[derive(Copy, Clone)]
pub struct ThemeView<'a> {
    pub theme: &'a Theme,
    pub overrides: &'a ThemeOverrides,
}

impl<'a> ThemeView<'a> {
    pub fn new(theme: &'a Theme, overrides: &'a ThemeOverrides) -> Self {
        Self { theme, overrides }
    }

    pub fn size(&self, sized: ThemeSized, tl: TreeTimeline) -> i32 {
        // Values are validated when the override is set.
        if let Some(v) = self.overrides.sizes[sized][tl].get() {
            return v;
        }
        sized.field(self.theme).get(tl)
    }

    pub fn color(&self, colored: ThemeColored, tl: TreeTimeline) -> Color {
        if let Some(v) = self.overrides.colors[colored][tl].get() {
            return v;
        }
        colored.field(self.theme).get()
    }

    fn color_is_set(&self, colored: ThemeColored, tl: TreeTimeline) -> bool {
        if self.overrides.colors[colored][tl].get().is_some() {
            return true;
        }
        colored.field(self.theme).set.get()
    }

    pub fn border_width(&self, tl: TreeTimeline) -> i32 {
        self.size(ThemeSized::border_width, tl)
    }

    pub fn show_titles(&self, tl: TreeTimeline) -> bool {
        self.theme.show_titles[tl].get()
    }

    pub fn title_height(&self, tl: TreeTimeline) -> i32 {
        if self.show_titles(tl) {
            self.size(ThemeSized::title_height, tl)
        } else {
            0
        }
    }

    pub fn title_icon_size(&self, tl: TreeTimeline) -> i32 {
        (self.title_height(tl) - 2).max(0)
    }

    pub fn title_underline_height(&self, tl: TreeTimeline) -> i32 {
        if self.show_titles(tl) { 1 } else { 0 }
    }

    pub fn title_plus_underline_height(&self, tl: TreeTimeline) -> i32 {
        if self.show_titles(tl) {
            self.size(ThemeSized::title_height, tl) + 1
        } else {
            0
        }
    }

    pub fn title_font(&self) -> Arc<String> {
        self.theme.title_font()
    }

    pub fn focused_border_color(&self, tl: TreeTimeline) -> Color {
        if self.color_is_set(ThemeColored::focused_border, tl) {
            self.color(ThemeColored::focused_border, tl)
        } else {
            self.color(ThemeColored::border, tl)
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

pub async fn handle_theme_changes(state: Rc<State>) {
    loop {
        state.theme_changed.triggered().await;
        let colors_changed = state.colors_changed.take();
        let spaces_changed = state.spaces_changed.take();
        if !colors_changed && !spaces_changed {
            continue;
        }
        struct V {
            colors_changed: bool,
            spaces_changed: bool,
        }
        macro_rules! trigger {
            ($slf:expr, $node:expr) => {
                if $slf.spaces_changed {
                    $node.on_spaces_changed();
                }
                if $slf.colors_changed {
                    $node.on_colors_changed();
                }
            };
        }
        impl NodeVisitorBase for V {
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                trigger!(self, node);
                node.node_visit_children(self);
            }
            fn visit_output(&mut self, node: &Rc<OutputNode>) {
                trigger!(self, node);
                node.node_visit_children(self);
            }
            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                trigger!(self, node);
                node.node_visit_children(self);
            }
        }
        let mut v = V {
            colors_changed,
            spaces_changed,
        };
        state.visit_all_nodes(&mut v);
        state.damage_full(LiveTL);
        state.damage_full(RenderTL);
        if colors_changed {
            state.icons.clear();
        }
        if spaces_changed {
            state.icons.update_sizes(&state);
            for client in state.clients.clients.borrow().values() {
                let mgrs = &client.data.objects.xdg_toplevel_icon_managers;
                for v in mgrs.lock().values() {
                    v.send_sizes();
                }
            }
            state.update_toplevel_icon_sizes();
        }
        state.trigger_cci(CCI_LOOK_AND_FEEL);
    }
}
