#![allow(clippy::excessive_precision)]

use {
    crate::{
        cmm::cmm_eotf::Eotf,
        format::ARGB8888,
        gfx_api::{GfxContext, GfxError, GfxTexture},
        scale::Scale,
        state::State,
        theme::Theme,
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
    icons: CopyHashMap<i32, Option<Rc<SizedIcons>>>,
}

#[derive(Copy, Clone, Debug, Linearize)]
pub enum IconState {
    Active,
    Passive,
}

pub struct SizedIcons {
    pub pin_unfocused_title: StaticMap<IconState, Rc<dyn GfxTexture>>,
    pub pin_focused_title: StaticMap<IconState, Rc<dyn GfxTexture>>,
    pub pin_attention_requested: StaticMap<IconState, Rc<dyn GfxTexture>>,
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
        let mut sizes = AHashSet::new();
        let height = state.theme.title_height();
        for &(scale, _) in &*state.scales.lock() {
            let [size] = scale.pixel_size([height]);
            if size > 0 {
                sizes.insert(size);
            }
        }
        self.icons.lock().retain(|size, _| sizes.contains(size));
    }

    pub fn clear(&self) {
        self.icons.clear();
    }

    pub fn get(&self, state: &State, scale: Scale) -> Option<Rc<SizedIcons>> {
        let [size] = scale.pixel_size([state.theme.title_height()]);
        if let Some(icons) = self.icons.get(&size) {
            return icons;
        }
        let icons = match self.create(state, size) {
            Ok(i) => Some(i),
            Err(e) => {
                log::error!("Could not create icons: {}", e);
                None
            }
        };
        self.icons.set(size, icons.clone());
        icons
    }

    fn create(&self, state: &State, size: i32) -> Result<Rc<SizedIcons>, IconsError> {
        let Some(ctx) = state.render_ctx.get() else {
            return Err(IconsError::NoRenderContext);
        };
        Ok(Rc::new(create_icons(size, &state.theme, &ctx)?))
    }
}

pub fn create_icons(
    size: i32,
    theme: &Theme,
    ctx: &Rc<dyn GfxContext>,
) -> Result<SizedIcons, IconsError> {
    if size <= 0 {
        return Err(IconsError::NonPositiveSize);
    }
    let size = size as u32;

    let create_pins = |color: crate::theme::Color| {
        let create_pin = |color: Color| {
            let mut paint = Paint::default();
            paint.set_color(color);
            let s = size as f32 / 100.0;
            let transform = Transform::from_scale(s, s);
            let mut pixmap = Pixmap::new(size, size).ok_or(IconsError::CreatePixmap)?;
            pixmap.fill_path(&PIN_PATH, &paint, FillRule::EvenOdd, transform, None);
            upload_pixmap(pixmap, ctx)
        };
        let colors = calculate_accents(color);
        Ok(static_map! {
            IconState::Passive => create_pin(colors[0])?,
            IconState::Active => create_pin(colors[1])?,
        })
    };

    Ok(SizedIcons {
        pin_unfocused_title: create_pins(theme.colors.unfocused_title_background.get())?,
        pin_focused_title: create_pins(theme.colors.focused_title_background.get())?,
        pin_attention_requested: create_pins(theme.colors.attention_requested_background.get())?,
    })
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
