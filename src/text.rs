use crate::format::ARGB8888;
use crate::pango::consts::{
    CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE, PANGO_ELLIPSIZE_END, PANGO_SCALE,
};
use crate::pango::{
    CairoContext, CairoImageSurface, PangoCairoContext, PangoError, PangoFontDescription,
    PangoLayout,
};
use crate::rect::Rect;
use crate::render::{RenderContext, RenderError, Texture};
use crate::theme::Color;
use std::ops::Neg;
use std::rc::Rc;
use thiserror::Error;

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

fn create_data(font: &str, width: i32, height: i32) -> Result<Data, TextError> {
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
    let fd = PangoFontDescription::from_string(font);
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

pub fn measure(font: &str, text: &str) -> Result<Rect, TextError> {
    let data = create_data(font, 1, 1)?;
    data.layout.set_text(text);
    Ok(data.layout.inc_pixel_rect())
}

pub fn render(
    ctx: &Rc<RenderContext>,
    width: i32,
    height: i32,
    font: &str,
    text: &str,
    color: Color,
) -> Result<Rc<Texture>, TextError> {
    render2(ctx, 1, width, height, 1, font, text, color, true)
}

pub fn render2(
    ctx: &Rc<RenderContext>,
    x: i32,
    width: i32,
    height: i32,
    padding: i32,
    font: &str,
    text: &str,
    color: Color,
    ellipsize: bool,
) -> Result<Rc<Texture>, TextError> {
    let data = create_data(font, width, height)?;
    if ellipsize {
        data.layout
            .set_width((width - 2 * padding).max(0) * PANGO_SCALE);
        data.layout.set_ellipsize(PANGO_ELLIPSIZE_END);
    }
    data.layout.set_text(text);
    let font_height = data.layout.pixel_size().1;
    data.cctx.set_operator(CAIRO_OPERATOR_SOURCE);
    data.cctx
        .set_source_rgba(color.r as _, color.g as _, color.b as _, color.a as _);
    data.cctx
        .move_to(x as f64, ((height - font_height) / 2) as f64);
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
    height: i32,
    font: &str,
    text: &str,
    color: Color,
) -> Result<Rc<Texture>, TextError> {
    let rect = measure(font, text)?;
    render2(
        ctx,
        rect.x1().neg(),
        rect.width(),
        height,
        0,
        font,
        text,
        color,
        false,
    )
}
