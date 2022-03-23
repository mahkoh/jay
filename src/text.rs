use crate::format::ARGB8888;
use crate::render::{RenderContext, Texture};
use crate::theme::Color;
use crate::RenderError;
use std::rc::Rc;
use thiserror::Error;
use crate::pango::{CairoImageSurface, PangoError, PangoFontDescription};
use crate::pango::consts::{CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE, PANGO_ELLIPSIZE_END, PANGO_SCALE};

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

pub fn render(
    ctx: &Rc<RenderContext>,
    width: i32,
    height: i32,
    font: &str,
    text: &str,
    color: Color,
) -> Result<Rc<Texture>, TextError> {
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
    layout.set_width((width - 2).max(0) * PANGO_SCALE);
    layout.set_ellipsize(PANGO_ELLIPSIZE_END);
    layout.set_font_description(&fd);
    layout.set_text(text);
    let font_height = layout.pixel_size().1;
    cctx.set_operator(CAIRO_OPERATOR_SOURCE);
    cctx.set_source_rgba(color.r as _, color.g as _, color.b as _, color.a as _);
    cctx.move_to(1.0, ((height - font_height) / 2) as f64);
    layout.show_layout();
    image.flush();
    let data = match image.data() {
        Ok(d) => d,
        Err(e) => return Err(TextError::ImageData(e)),
    };
    match ctx.shmem_texture(data, ARGB8888, width, height, image.stride()) {
        Ok(t) => Ok(t),
        Err(e) => Err(TextError::RenderError(e)),
    }
}
