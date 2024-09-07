use {
    crate::{
        format::ARGB8888,
        gfx_api::{GfxContext, GfxError, GfxTexture, ShmGfxTexture},
        pango::{
            consts::{
                CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE, PANGO_ELLIPSIZE_END, PANGO_SCALE,
            },
            CairoContext, CairoImageSurface, PangoCairoContext, PangoError, PangoFontDescription,
            PangoLayout,
        },
        rect::Rect,
        theme::Color,
        utils::clonecell::UnsafeCellCloneSafe,
    },
    std::{
        borrow::Cow,
        ops::{Deref, Neg},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum TextError {
    #[error("Could not create a cairo image")]
    CreateImage(#[source] PangoError),
    #[error("Could not create a cairo context")]
    CairoContext(#[source] PangoError),
    #[error("Could not create a pango context")]
    PangoContext(#[source] PangoError),
    #[error("Could not create a pango layout")]
    CreateLayout(#[source] PangoError),
    #[error("Could not import the rendered text")]
    RenderError(#[source] GfxError),
    #[error("Could not access the cairo image data")]
    ImageData(#[source] PangoError),
}

#[derive(PartialEq)]
struct Config<'a> {
    x: i32,
    y: Option<i32>,
    width: i32,
    height: i32,
    padding: i32,
    font: Cow<'a, str>,
    text: Cow<'a, str>,
    color: Color,
    ellipsize: bool,
    markup: bool,
    scale: Option<f64>,
}

impl<'a> Config<'a> {
    fn to_static(self) -> Config<'static> {
        Config {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            padding: self.padding,
            font: Cow::Owned(self.font.into_owned()),
            text: Cow::Owned(self.text.into_owned()),
            color: self.color,
            ellipsize: self.ellipsize,
            markup: self.markup,
            scale: self.scale,
        }
    }
}

#[derive(Clone)]
pub struct TextTexture {
    config: Rc<Config<'static>>,
    shm_texture: Rc<dyn ShmGfxTexture>,
    pub texture: Rc<dyn GfxTexture>,
}

unsafe impl UnsafeCellCloneSafe for TextTexture {}

struct Data {
    image: Rc<CairoImageSurface>,
    cctx: Rc<CairoContext>,
    _pctx: Rc<PangoCairoContext>,
    _fd: PangoFontDescription,
    layout: PangoLayout,
}

fn create_data(font: &str, width: i32, height: i32, scale: Option<f64>) -> Result<Data, TextError> {
    let image = match CairoImageSurface::new_image_surface(CAIRO_FORMAT_ARGB32, width, height) {
        Ok(s) => s,
        Err(e) => return Err(TextError::CreateImage(e)),
    };
    let cctx = match image.create_context() {
        Ok(c) => c,
        Err(e) => return Err(TextError::CairoContext(e)),
    };
    let pctx = match cctx.create_pango_context() {
        Ok(c) => c,
        Err(e) => return Err(TextError::PangoContext(e)),
    };
    let mut fd = PangoFontDescription::from_string(font);
    if let Some(scale) = scale {
        fd.set_size((fd.size() as f64 * scale).round() as _);
    }
    let layout = match pctx.create_layout() {
        Ok(l) => l,
        Err(e) => return Err(TextError::CreateLayout(e)),
    };
    layout.set_font_description(&fd);
    Ok(Data {
        image,
        cctx,
        _pctx: pctx,
        _fd: fd,
        layout,
    })
}

pub fn measure(
    font: &str,
    text: &str,
    markup: bool,
    scale: Option<f64>,
    full: bool,
) -> Result<TextMeasurement, TextError> {
    let data = create_data(font, 1, 1, scale)?;
    if markup {
        data.layout.set_markup(text);
    } else {
        data.layout.set_text(text);
    }
    let mut res = TextMeasurement::default();
    res.ink_rect = data.layout.inc_pixel_rect();
    if full {
        res.logical_rect = data.layout.logical_pixel_rect();
        res.baseline = data.layout.pixel_baseline();
    }
    Ok(res)
}

pub fn render(
    ctx: &Rc<dyn GfxContext>,
    old: Option<TextTexture>,
    width: i32,
    height: i32,
    font: &str,
    text: &str,
    color: Color,
    scale: Option<f64>,
) -> Result<TextTexture, TextError> {
    render2(
        ctx, old, 1, None, width, height, 1, font, text, color, true, false, scale,
    )
}

fn render2(
    ctx: &Rc<dyn GfxContext>,
    old: Option<TextTexture>,
    x: i32,
    y: Option<i32>,
    width: i32,
    height: i32,
    padding: i32,
    font: &str,
    text: &str,
    color: Color,
    ellipsize: bool,
    markup: bool,
    scale: Option<f64>,
) -> Result<TextTexture, TextError> {
    let config = Config {
        x,
        y,
        width,
        height,
        padding,
        font: Cow::Borrowed(font),
        text: Cow::Borrowed(text),
        color,
        ellipsize,
        markup,
        scale,
    };
    if let Some(old2) = &old {
        if old2.config.deref() == &config {
            return Ok(old.unwrap());
        }
    }
    let data = create_data(font, width, height, scale)?;
    if ellipsize {
        data.layout
            .set_width((width - 2 * padding).max(0) * PANGO_SCALE);
        data.layout.set_ellipsize(PANGO_ELLIPSIZE_END);
    }
    if markup {
        data.layout.set_markup(text);
    } else {
        data.layout.set_text(text);
    }
    let font_height = data.layout.pixel_size().1;
    data.cctx.set_operator(CAIRO_OPERATOR_SOURCE);
    data.cctx
        .set_source_rgba(color.r as _, color.g as _, color.b as _, color.a as _);
    let y = y.unwrap_or((height - font_height) / 2);
    data.cctx.move_to(x as f64, y as f64);
    data.layout.show_layout();
    data.image.flush();
    let bytes = match data.image.data() {
        Ok(d) => d,
        Err(e) => return Err(TextError::ImageData(e)),
    };
    let old = old.map(|o| o.shm_texture);
    match ctx.clone().shmem_texture(
        old,
        bytes,
        ARGB8888,
        width,
        height,
        data.image.stride(),
        None,
    ) {
        Ok(t) => Ok(TextTexture {
            config: Rc::new(config.to_static()),
            texture: t.clone().into_texture(),
            shm_texture: t,
        }),
        Err(e) => Err(TextError::RenderError(e)),
    }
}

pub fn render_fitting(
    ctx: &Rc<dyn GfxContext>,
    old: Option<TextTexture>,
    height: Option<i32>,
    font: &str,
    text: &str,
    color: Color,
    markup: bool,
    scale: Option<f64>,
) -> Result<TextTexture, TextError> {
    render_fitting2(ctx, old, height, font, text, color, markup, scale, false).map(|(a, _)| a)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TextMeasurement {
    pub ink_rect: Rect,
    pub logical_rect: Rect,
    pub baseline: i32,
}

pub fn render_fitting2(
    ctx: &Rc<dyn GfxContext>,
    old: Option<TextTexture>,
    height: Option<i32>,
    font: &str,
    text: &str,
    color: Color,
    markup: bool,
    scale: Option<f64>,
    include_measurements: bool,
) -> Result<(TextTexture, TextMeasurement), TextError> {
    let measurement = measure(font, text, markup, scale, include_measurements)?;
    let y = match height {
        Some(_) => None,
        _ => Some(measurement.ink_rect.y1().neg()),
    };
    let res = render2(
        ctx,
        old,
        measurement.ink_rect.x1().neg(),
        y,
        measurement.ink_rect.width(),
        height.unwrap_or(measurement.ink_rect.height()),
        0,
        font,
        text,
        color,
        false,
        markup,
        scale,
    );
    res.map(|r| (r, measurement))
}
