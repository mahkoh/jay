use {
    crate::{
        format::ARGB8888,
        pango::{
            consts::{
                CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE, PANGO_ELLIPSIZE_END, PANGO_SCALE,
            },
            CairoContext, CairoImageSurface, PangoCairoContext, PangoError, PangoFontDescription,
            PangoLayout,
        },
        rect::Rect,
        render::{RenderContext, RenderError, Texture},
        theme::Color,
    },
    std::{ops::Neg, rc::Rc},
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
    RenderError(#[source] RenderError),
    #[error("Could not access the cairo image data")]
    ImageData(#[source] PangoError),
}

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
    ctx: &Rc<RenderContext>,
    width: i32,
    height: i32,
    font: &str,
    text: &str,
    color: Color,
    scale: Option<f64>,
) -> Result<Rc<Texture>, TextError> {
    render2(
        ctx, 1, None, width, height, 1, font, text, color, true, false, scale,
    )
}

fn render2(
    ctx: &Rc<RenderContext>,
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
) -> Result<Rc<Texture>, TextError> {
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
    match ctx.shmem_texture(bytes, ARGB8888, width, height, data.image.stride()) {
        Ok(t) => Ok(t),
        Err(e) => Err(TextError::RenderError(e)),
    }
}

pub fn render_fitting(
    ctx: &Rc<RenderContext>,
    height: Option<i32>,
    font: &str,
    text: &str,
    color: Color,
    markup: bool,
    scale: Option<f64>,
) -> Result<Rc<Texture>, TextError> {
    render_fitting2(ctx, height, font, text, color, markup, scale, false).map(|(a, b)| a)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TextMeasurement {
    pub ink_rect: Rect,
    pub logical_rect: Rect,
    pub baseline: i32,
}

pub fn render_fitting2(
    ctx: &Rc<RenderContext>,
    height: Option<i32>,
    font: &str,
    text: &str,
    color: Color,
    markup: bool,
    scale: Option<f64>,
    include_measurements: bool,
) -> Result<(Rc<Texture>, TextMeasurement), TextError> {
    let measurement = measure(font, text, markup, scale, include_measurements)?;
    log::info!("measurement = {:?}", measurement);
    let y = match height {
        Some(_) => None,
        _ => Some(measurement.ink_rect.y1().neg()),
    };
    let res = render2(
        ctx,
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
