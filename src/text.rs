use crate::format::ARGB8888;
use crate::render::{RenderContext, Texture};
use crate::theme::Color;
use crate::RenderError;
use cairo::{ImageSurface, Operator};
use pango::{EllipsizeMode, Layout};
use pangocairo::cairo::Format;
use pangocairo::pango::FontDescription;
use pangocairo::{cairo, pango};
use std::mem;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextError {
    #[error("Could not create a cairo image")]
    CreateImage(#[source] cairo::Error),
    #[error("Could not create a cairo context")]
    CairoContext(#[source] cairo::Error),
    #[error("Could not create a pango context")]
    PangoContext,
    #[error("Could not import the rendered text")]
    RenderError(#[source] RenderError),
    #[error("Could not access the cairo image data")]
    ImageData,
}

pub fn render(
    ctx: &Rc<RenderContext>,
    width: i32,
    height: i32,
    font: &str,
    text: &str,
    color: Color,
) -> Result<Rc<Texture>, TextError> {
    let image = match ImageSurface::create(Format::ARgb32, width, height) {
        Ok(s) => s,
        Err(e) => return Err(TextError::CreateImage(e)),
    };
    let cctx = match cairo::Context::new(&image) {
        Ok(c) => c,
        Err(e) => return Err(TextError::CairoContext(e)),
    };
    let pctx = match pangocairo::create_context(&cctx) {
        Some(c) => c,
        _ => return Err(TextError::PangoContext),
    };
    let fd = FontDescription::from_string(font);
    let layout = Layout::new(&pctx);
    layout.set_width((width - 2).max(0) * pango::SCALE);
    layout.set_ellipsize(EllipsizeMode::End);
    layout.set_font_description(Some(&fd));
    layout.set_text(text);
    let font_height = layout.pixel_size().1;
    cctx.set_operator(Operator::Source);
    cctx.set_source_rgba(color.r as _, color.g as _, color.b as _, color.a as _);
    cctx.move_to(1.0, ((height - font_height) / 2) as f64);
    pangocairo::show_layout(&cctx, &layout);
    let mut texture = None;
    let _ = image.with_data(|d| unsafe {
        let d = mem::transmute(d);
        texture = Some(ctx.shmem_texture(d, ARGB8888, width, height, image.stride()));
    });
    match texture {
        Some(Ok(t)) => Ok(t),
        Some(Err(e)) => Err(TextError::RenderError(e)),
        None => Err(TextError::ImageData),
    }
}
