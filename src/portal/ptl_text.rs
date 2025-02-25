use {
    crate::{
        format::ARGB8888,
        gfx_api::{GfxContext, GfxTexture},
        pango::{
            CairoContext, CairoImageSurface, PangoCairoContext, PangoFontDescription, PangoLayout,
            consts::{CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE},
        },
        rect::Rect,
        theme::{Color, TransferFunction},
    },
    std::{ops::Neg, rc::Rc, sync::Arc},
};

struct Data {
    image: Rc<CairoImageSurface>,
    cctx: Rc<CairoContext>,
    _pctx: Rc<PangoCairoContext>,
    _fd: PangoFontDescription,
    layout: PangoLayout,
}

fn create_data(font: &str, width: i32, height: i32, scale: Option<f64>) -> Option<Data> {
    let image = CairoImageSurface::new_image_surface(CAIRO_FORMAT_ARGB32, width, height).ok()?;
    let cctx = image.create_context().ok()?;
    let pctx = cctx.create_pango_context().ok()?;
    let mut fd = PangoFontDescription::from_string(font);
    if let Some(scale) = scale {
        fd.set_size((fd.size() as f64 * scale).round() as _);
    }
    let layout = pctx.create_layout().ok()?;
    layout.set_font_description(&fd);
    Some(Data {
        image,
        cctx,
        _pctx: pctx,
        _fd: fd,
        layout,
    })
}

fn measure(font: &str, text: &str, scale: Option<f64>, full: bool) -> Option<TextMeasurement> {
    let data = create_data(font, 1, 1, scale)?;
    data.layout.set_text(text);
    let mut res = TextMeasurement::default();
    res.ink_rect = data.layout.inc_pixel_rect();
    if full {
        res.logical_rect = data.layout.logical_pixel_rect();
        res.baseline = data.layout.pixel_baseline();
    }
    Some(res)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TextMeasurement {
    pub ink_rect: Rect,
    pub logical_rect: Rect,
    pub baseline: i32,
}

pub fn render(
    ctx: &Rc<dyn GfxContext>,
    height: Option<i32>,
    font: &Arc<String>,
    text: &str,
    color: Color,
    scale: Option<f64>,
    include_measurements: bool,
) -> Option<(Rc<dyn GfxTexture>, TextMeasurement)> {
    let measurement = measure(font, text, scale, include_measurements)?;
    let y = match height {
        Some(_) => None,
        _ => Some(measurement.ink_rect.y1().neg()),
    };
    let x = measurement.ink_rect.x1().neg();
    let width = measurement.ink_rect.width();
    let height = height.unwrap_or(measurement.ink_rect.height());
    let data = create_data(font, width, height, scale)?;
    data.layout.set_text(text);
    let font_height = data.layout.pixel_size().1;
    let [r, g, b, a] = color.to_array(TransferFunction::Srgb);
    data.cctx.set_operator(CAIRO_OPERATOR_SOURCE);
    data.cctx.set_source_rgba(r as _, g as _, b as _, a as _);
    let y = y.unwrap_or((height - font_height) / 2);
    data.cctx.move_to(x as f64, y as f64);
    data.layout.show_layout();
    data.image.flush();
    let bytes = data.image.data().ok()?;
    ctx.clone()
        .shmem_texture(
            None,
            bytes,
            ARGB8888,
            width,
            height,
            data.image.stride(),
            None,
        )
        .ok()
        .map(|t| (t.into_texture(), measurement))
}
