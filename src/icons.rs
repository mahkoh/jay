#![allow(clippy::excessive_precision)]

use {
    crate::{
        cmm::cmm_eotf::Eotf,
        format::ARGB8888,
        gfx_api::{GfxContext, GfxError, GfxTexture},
        scale::Scale,
        state::State,
        theme::Theme,
        tree::TreeTimeline::LiveTL,
        utils::{copyhashmap::CopyHashMap, windows::WindowsExt},
    },
    ahash::AHashSet,
    linearize::{Linearize, StaticMap, static_map},
    std::{cell::Cell, f32::consts::PI, mem, rc::Rc, sync::LazyLock},
    thiserror::Error,
    tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Transform},
};

#[derive(Default)]
pub struct Icons {
    title_icons: CopyHashMap<i32, Option<Rc<SizedTitleIcons>>>,
    bar_icons: CopyHashMap<i32, Option<Rc<SizedBarIcons>>>,
    compositing_icon: CopyHashMap<i32, Option<Rc<SizedCompositingIcon>>>,
}

#[derive(Copy, Clone, Debug, Linearize)]
pub enum IconState {
    Active,
    Passive,
}

pub struct SizedTitleIcons {
    pub pin_unfocused_title: StaticMap<IconState, Rc<dyn GfxTexture>>,
    pub pin_focused_title: StaticMap<IconState, Rc<dyn GfxTexture>>,
    pub pin_attention_requested: StaticMap<IconState, Rc<dyn GfxTexture>>,
    pub overlay_unfocused_title: Rc<dyn GfxTexture>,
    pub overlay_focused_title: Rc<dyn GfxTexture>,
    pub overlay_focused_inactive_title: Rc<dyn GfxTexture>,
    pub overlay_attention_requested: Rc<dyn GfxTexture>,
}

pub struct SizedBarIcons {
    pub overlay: Rc<dyn GfxTexture>,
}

pub struct SizedCompositingIcon {
    pub icon: Rc<dyn GfxTexture>,
}

#[derive(Debug, Error)]
pub enum IconsError {
    #[error("Could not create a pixmap")]
    CreatePixmap,
    #[error("The requested icons size is non-positive")]
    NonPositiveSize,
    #[error("There is no gfx context")]
    NoRenderContext,
    #[error("Could not create texture")]
    CreateTexture(#[source] GfxError),
}

impl Icons {
    pub fn update_sizes(&self, state: &State) {
        self.update_sizes_(state, state.theme.title_height(LiveTL), &self.title_icons);
        self.update_sizes_(state, state.theme.sizes.bar_height(LiveTL), &self.bar_icons);
        self.update_sizes_(state, 100, &self.compositing_icon);
    }

    fn update_sizes_(&self, state: &State, height: i32, map: &CopyHashMap<i32, impl Sized>) {
        let mut sizes = AHashSet::new();
        for &(scale, _) in &*state.scales.lock() {
            let [size] = scale.pixel_size([height]);
            if size > 0 {
                sizes.insert(size);
            }
        }
        map.lock().retain(|size, _| sizes.contains(size));
    }

    pub fn clear(&self) {
        self.title_icons.clear();
        self.bar_icons.clear();
        self.compositing_icon.clear();
    }

    pub fn get_title_icons(&self, state: &State, scale: Scale) -> Option<Rc<SizedTitleIcons>> {
        self.get(
            state,
            scale,
            state.theme.title_height(LiveTL),
            &self.title_icons,
            create_title_icons,
        )
    }

    pub fn get_bar_icons(&self, state: &State, scale: Scale) -> Option<Rc<SizedBarIcons>> {
        self.get(
            state,
            scale,
            state.theme.sizes.bar_height(LiveTL),
            &self.bar_icons,
            create_bar_icons,
        )
    }

    pub fn get_compositing_icon(
        &self,
        state: &State,
        scale: Scale,
    ) -> Option<Rc<SizedCompositingIcon>> {
        self.get(
            state,
            scale,
            100,
            &self.compositing_icon,
            create_compositing_icon,
        )
    }

    fn get<T>(
        &self,
        state: &State,
        scale: Scale,
        height: i32,
        map: &CopyHashMap<i32, Option<Rc<T>>>,
        f: impl FnOnce(i32, &Theme, &Rc<dyn GfxContext>) -> Result<T, IconsError>,
    ) -> Option<Rc<T>> {
        let [size] = scale.pixel_size([height]);
        if let Some(icons) = map.get(&size) {
            return icons;
        }
        let icons = match self.create(state, size, f) {
            Ok(i) => Some(i),
            Err(e) => {
                log::error!("Could not create icons: {}", e);
                None
            }
        };
        map.set(size, icons.clone());
        icons
    }

    fn create<T>(
        &self,
        state: &State,
        size: i32,
        f: impl FnOnce(i32, &Theme, &Rc<dyn GfxContext>) -> Result<T, IconsError>,
    ) -> Result<Rc<T>, IconsError> {
        let Some(ctx) = state.render_ctx.get() else {
            return Err(IconsError::NoRenderContext);
        };
        Ok(Rc::new(f(size, &state.theme, &ctx)?))
    }
}

fn create_icon(
    size: u32,
    ctx: &Rc<dyn GfxContext>,
    path: &Path,
    color: Color,
) -> Result<Rc<dyn GfxTexture>, IconsError> {
    let mut paint = Paint::default();
    paint.set_color(color);
    let s = size as f32 / 100.0;
    let transform = Transform::from_scale(s, s);
    let mut pixmap = Pixmap::new(size, size).ok_or(IconsError::CreatePixmap)?;
    pixmap.fill_path(path, &paint, FillRule::EvenOdd, transform, None);
    upload_pixmap(pixmap, ctx)
}

pub fn create_title_icons(
    size: i32,
    theme: &Theme,
    ctx: &Rc<dyn GfxContext>,
) -> Result<SizedTitleIcons, IconsError> {
    if size <= 0 {
        return Err(IconsError::NonPositiveSize);
    }
    let size = size as u32;

    let create_icon = |path: &Path, color: Color| create_icon(size, ctx, path, color);
    let create_pins = |color: crate::theme::Color| {
        let colors = calculate_accents(color);
        Ok(static_map! {
            IconState::Passive => create_icon(&PIN_PATH, colors[0])?,
            IconState::Active => create_icon(&PIN_PATH, colors[1])?,
        })
    };
    let create_overlay = |color: crate::theme::Color| {
        let colors = calculate_accents(color);
        create_icon(&OVERLAY_PATH, colors[0])
    };

    Ok(SizedTitleIcons {
        pin_unfocused_title: create_pins(theme.colors.unfocused_title_background.get())?,
        pin_focused_title: create_pins(theme.colors.focused_title_background.get())?,
        pin_attention_requested: create_pins(theme.colors.attention_requested_background.get())?,
        overlay_unfocused_title: create_overlay(theme.colors.unfocused_title_background.get())?,
        overlay_attention_requested: create_overlay(
            theme.colors.attention_requested_background.get(),
        )?,
        overlay_focused_title: create_overlay(theme.colors.focused_title_background.get())?,
        overlay_focused_inactive_title: create_overlay(
            theme.colors.focused_inactive_title_background.get(),
        )?,
    })
}

pub fn create_bar_icons(
    size: i32,
    theme: &Theme,
    ctx: &Rc<dyn GfxContext>,
) -> Result<SizedBarIcons, IconsError> {
    if size <= 0 {
        return Err(IconsError::NonPositiveSize);
    }
    let overlay = create_icon(
        size as u32,
        ctx,
        &OVERLAY_PATH,
        calculate_accents(theme.colors.focused_title_background.get())[0],
    )?;
    Ok(SizedBarIcons { overlay })
}

pub fn create_compositing_icon(
    size: i32,
    _theme: &Theme,
    ctx: &Rc<dyn GfxContext>,
) -> Result<SizedCompositingIcon, IconsError> {
    if size <= 0 {
        return Err(IconsError::NonPositiveSize);
    }
    let mut fg = Color::WHITE;
    fg.set_alpha(0.5);
    let mut bg = Color::BLACK;
    bg.set_alpha(0.5);
    let mut paint = Paint::default();
    paint.set_color(fg);
    let size = size as u32;
    let s = size as f32 / 100.0;
    let transform = Transform::from_scale(s, s);
    let mut pixmap = Pixmap::new(size, size).ok_or(IconsError::CreatePixmap)?;
    pixmap.fill(bg);
    pixmap.fill_path(
        &COMPOSITING_PATH,
        &paint,
        FillRule::EvenOdd,
        transform,
        None,
    );
    let icon = upload_pixmap(pixmap, ctx)?;
    Ok(SizedCompositingIcon { icon })
}

fn upload_pixmap(
    pixmap: Pixmap,
    ctx: &Rc<dyn GfxContext>,
) -> Result<Rc<dyn GfxTexture>, IconsError> {
    let width = pixmap.width();
    let height = pixmap.width();
    let bytes = unsafe { mem::transmute::<Vec<u8>, Vec<Cell<u8>>>(pixmap.take()) };
    for chunk in bytes.array_chunks_ext::<4>() {
        let r = chunk[0].get();
        let b = chunk[2].get();
        chunk[0].set(b);
        chunk[2].set(r);
    }
    let tex: Rc<dyn GfxTexture> = ctx
        .clone()
        .shmem_texture(
            None,
            &bytes,
            ARGB8888,
            width as _,
            height as _,
            width as i32 * 4,
            None,
        )
        .map_err(IconsError::CreateTexture)?;
    Ok(tex)
}

static PIN_PATH: LazyLock<Path> = LazyLock::new(|| {
    let cx = 50.0f32;
    let cy = 40.0f32;
    let r = 30.0f32;
    let xx = cx;
    let xy = 90.0f32;
    let d = xy - cy;
    let v1 = r / d * (d * d - r * r).sqrt();
    let v2 = 1.0 / d * (d * d - r * r);

    let mut path = PathBuilder::new();
    path.move_to(cx, cy - r);
    path.arc_cw_to(cx, cy, cx + r, cy);
    path.arc_cw_to(cx, cy, xx + v1, xy - v2);
    path.line_to(xx, xy);
    path.line_to(xx - v1, xy - v2);
    path.arc_cw_to(cx, cy, cx - r, cy);
    path.arc_cw_to(cx, cy, cx, cy - r);
    path.close();
    path.push_circle(cx, cy, r / 2.5);
    path.finish().unwrap()
});

#[test]
fn pin_path() {
    let _path = &*PIN_PATH;
}

static OVERLAY_PATH: LazyLock<Path> = LazyLock::new(|| {
    const UT: f32 = 12.0;
    const CX: f32 = 50.0;
    const CY: f32 = 42.0;
    const W: f32 = 40.0;
    const H: f32 = 30.0;
    const LT: f32 = 8.0;

    let alpha = (H / W).atan();
    let udy = UT / alpha.cos();
    let udx = UT / alpha.sin();
    let ldy = LT / alpha.cos();
    let ldy2 = 0.5 * ldy;
    let ldy3 = 1.5 * ldy;
    let idy = LT * alpha.sin();
    let idx = LT * alpha.cos();

    let mut path = PathBuilder::new();
    path.move_to(CX, CY - H);
    path.line_to(CX + W, CY);
    path.line_to(CX, CY + H);
    path.line_to(CX - W, CY);
    path.close();
    path.move_to(CX, CY - H + udy);
    path.line_to(CX + W - udx, CY);
    path.line_to(CX, CY + H - udy);
    path.line_to(CX - W + udx, CY);
    path.close();
    path.move_to(CX + W, CY + ldy3);
    path.line_to(CX, CY + H + ldy3);
    path.line_to(CX - W, CY + ldy3);
    path.line_to(CX - W + idx, CY + ldy3 - idy);
    path.line_to(CX, CY + H + ldy2);
    path.line_to(CX + W - idx, CY + ldy3 - idy);
    path.close();
    path.finish().unwrap()
});

#[test]
fn overlay_path() {
    let _path = &*OVERLAY_PATH;
}

static COMPOSITING_PATH: LazyLock<Path> = LazyLock::new(|| {
    let mut path = PathBuilder::new();
    let mut draw_square = |d: f32| {
        path.move_to(5.0 + d, 5.0 + d);
        path.line_to(95.0 - d, 5.0 + d);
        path.line_to(95.0 - d, 95.0 - d);
        path.line_to(5.0 + d, 95.0 - d);
        path.close();
    };
    draw_square(0.0);
    draw_square(10.0);
    let mut draw_c = |d: f32| {
        path.move_to(80.0 - d, 80.0);
        path.arc_cw_to(80.0 - d, 50.0, 50.0 - d, 50.0);
        path.arc_cw_to(80.0 - d, 50.0, 80.0 - d, 20.0);
        path.line_to(80.0 - d, 30.0);
        path.move_to(80.0 - d, 70.0);
        path.arc_cw_to(80.0 - d, 50.0, 60.0 - d, 50.0);
        path.arc_cw_to(80.0 - d, 50.0, 80.0 - d, 30.0);
        path.close();
    };
    draw_c(0.0);
    draw_c(30.0);
    path.finish().unwrap()
});

#[test]
fn compositing_path() {
    let _path = &*COMPOSITING_PATH;
}

trait PathBuilderExt {
    fn arc_cw_to(&mut self, cx: f32, cy: f32, x: f32, y: f32);
}

impl PathBuilderExt for PathBuilder {
    fn arc_cw_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (x0, y0) = match self.last_point() {
            None => {
                self.move_to(0.0, 0.0);
                (0.0, 0.0)
            }
            Some(p) => (p.x, p.y),
        };

        let ux = x0 - cx;
        let uy = y0 - cy;
        let ul = (ux * ux + uy * uy).sqrt();
        let uxn = ux / ul;
        let uyn = uy / ul;
        let a1 = (uy / ux).atan();

        let tx = x - cx;
        let ty = y - cy;
        let tl = (tx * tx + ty * ty).sqrt();
        let txn = tx / tl;
        let tyn = ty / tl;
        let a2 = (ty / tx).atan();

        let c = 4.0 / 3.0 * ((a2 - a1 + PI) % PI / 4.0).tan();
        let uc = ul * c;
        let tc = tl * c;
        self.cubic_to(
            x0 - uyn * uc,
            y0 + uxn * uc,
            x + tyn * tc,
            y - txn * tc,
            x,
            y,
        );
    }
}

impl From<crate::theme::Color> for Color {
    fn from(v: crate::theme::Color) -> Self {
        let [r, g, b, a] = v.to_array(Eotf::Gamma22);
        let mut c = Self::TRANSPARENT;
        c.set_red(r / a);
        c.set_green(g / a);
        c.set_blue(b / a);
        c.set_alpha(a);
        c
    }
}

fn calculate_accents(srgb: crate::theme::Color) -> [Color; 2] {
    let [l, a, b, alpha] = srgb_to_lab(srgb);
    let l2 = if l < 0.65 { 0.9 } else { l - 0.4 };
    let l1 = (l2 + l) / 2.0;
    [
        lab_to_color([l1, a, b, alpha]),
        lab_to_color([l2, a, b, alpha]),
    ]
}

fn srgb_to_lab(srgb: crate::theme::Color) -> [f32; 4] {
    let [mut r, mut g, mut b, alpha] = srgb.to_array(Eotf::Gamma22);
    if alpha < 1.0 {
        r /= alpha;
        g /= alpha;
        b /= alpha;
    }

    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    [
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
        alpha,
    ]
}

fn lab_to_color([l, a, b, alpha]: [f32; 4]) -> Color {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let mut c = Color::TRANSPARENT;
    c.set_red(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
    c.set_green(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
    c.set_blue(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s);
    c.set_alpha(alpha);
    c
}
