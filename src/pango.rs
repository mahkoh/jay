#![allow(non_camel_case_types)]

use {
    crate::{
        pango::consts::{CairoFormat, CairoOperator, PangoEllipsizeMode},
        rect::Rect,
    },
    std::{cell::Cell, ptr, rc::Rc},
    thiserror::Error,
    uapi::{c, IntoUstr},
};

pub mod consts;

include!(concat!(env!("OUT_DIR"), "/pango_tys.rs"));

#[link(name = "cairo")]
extern "C" {
    type cairo_surface_t;
    type cairo_t;

    fn cairo_image_surface_create(
        format: cairo_format_t,
        width: c::c_int,
        height: c::c_int,
    ) -> *mut cairo_surface_t;
    fn cairo_image_surface_get_height(surface: *mut cairo_surface_t) -> c::c_int;
    fn cairo_image_surface_get_stride(surface: *mut cairo_surface_t) -> c::c_int;
    fn cairo_image_surface_get_data(surface: *mut cairo_surface_t) -> *mut u8;

    fn cairo_surface_destroy(surface: *mut cairo_surface_t);
    fn cairo_surface_status(surface: *mut cairo_surface_t) -> cairo_status_t;
    fn cairo_surface_flush(surface: *mut cairo_surface_t);

    fn cairo_create(surface: *mut cairo_surface_t) -> *mut cairo_t;
    fn cairo_status(cairo: *mut cairo_t) -> cairo_status_t;
    fn cairo_destroy(cairo: *mut cairo_t);

    fn cairo_set_operator(cr: *mut cairo_t, op: cairo_operator_t);
    fn cairo_set_source_rgba(cr: *mut cairo_t, red: f64, green: f64, blue: f64, alpha: f64);
    fn cairo_move_to(cr: *mut cairo_t, x: f64, y: f64);
}

#[link(name = "pangocairo-1.0")]
extern "C" {
    type PangoContext_;

    fn pango_cairo_create_context(cr: *mut cairo_t) -> *mut PangoContext_;
    fn pango_cairo_show_layout(cr: *mut cairo_t, layout: *mut PangoLayout_);
}

#[link(name = "gobject-2.0")]
extern "C" {
    type GObject;

    fn g_object_unref(object: *mut GObject);
}

#[link(name = "pango-1.0")]
extern "C" {
    type PangoFontDescription_;
    type PangoLayout_;

    fn pango_font_description_from_string(str: *const c::c_char) -> *mut PangoFontDescription_;
    fn pango_font_description_free(desc: *mut PangoFontDescription_);
    fn pango_font_description_get_size(desc: *mut PangoFontDescription_) -> c::c_int;
    fn pango_font_description_set_size(desc: *mut PangoFontDescription_, size: c::c_int);

    fn pango_layout_new(context: *mut PangoContext_) -> *mut PangoLayout_;
    fn pango_layout_set_width(layout: *mut PangoLayout_, width: c::c_int);
    fn pango_layout_set_ellipsize(layout: *mut PangoLayout_, ellipsize: PangoEllipsizeMode_);
    fn pango_layout_set_font_description(
        layout: *mut PangoLayout_,
        desc: *const PangoFontDescription_,
    );
    fn pango_layout_set_text(layout: *mut PangoLayout_, text: *const c::c_char, length: c::c_int);
    fn pango_layout_set_markup(layout: *mut PangoLayout_, text: *const c::c_char, length: c::c_int);
    fn pango_layout_get_pixel_size(
        layout: *mut PangoLayout_,
        width: *mut c::c_int,
        height: *mut c::c_int,
    );
    fn pango_layout_get_extents(
        layout: *mut PangoLayout_,
        ink_rect: *mut PangoRectangle,
        logical_rect: *mut PangoRectangle,
    );

    fn pango_extents_to_pixels(inclusive: *mut PangoRectangle, nearest: *mut PangoRectangle);
}

#[derive(Debug, Error)]
pub enum PangoError {
    #[error("Could not create an image surface: {0}")]
    CreateSurface(u32),
    #[error("Could not create a cairo context: {0}")]
    CreateCairo(u32),
    #[error("Could not create a pangocairo context")]
    CreatePangoCairo,
    #[error("Could not create a pango layout")]
    CreateLayout,
    #[error("Could not retrieve image data")]
    GetData,
}

#[repr(C)]
struct PangoRectangle {
    x: c::c_int,
    y: c::c_int,
    width: c::c_int,
    height: c::c_int,
}

pub struct CairoImageSurface {
    s: *mut cairo_surface_t,
}

impl CairoImageSurface {
    pub fn new_image_surface(
        format: CairoFormat,
        width: i32,
        height: i32,
    ) -> Result<Rc<Self>, PangoError> {
        unsafe {
            let s = cairo_image_surface_create(format.raw() as _, width as _, height as _);
            let status = cairo_surface_status(s);
            if status != 0 {
                return Err(PangoError::CreateSurface(status as _));
            }
            Ok(Rc::new(Self { s }))
        }
    }

    pub fn create_context(self: &Rc<Self>) -> Result<Rc<CairoContext>, PangoError> {
        unsafe {
            let c = cairo_create(self.s);
            let status = cairo_status(c);
            if status != 0 {
                return Err(PangoError::CreateCairo(status as _));
            }
            Ok(Rc::new(CairoContext {
                _s: self.clone(),
                c,
            }))
        }
    }

    pub fn flush(&self) {
        unsafe {
            cairo_surface_flush(self.s);
        }
    }

    pub fn height(&self) -> i32 {
        unsafe { cairo_image_surface_get_height(self.s) as _ }
    }

    pub fn stride(&self) -> i32 {
        unsafe { cairo_image_surface_get_stride(self.s) as _ }
    }

    pub fn data(&self) -> Result<&[Cell<u8>], PangoError> {
        unsafe {
            let d = cairo_image_surface_get_data(self.s);
            if d.is_null() {
                return Err(PangoError::GetData);
            }
            let size = self.height() as usize * self.stride() as usize;
            Ok(std::slice::from_raw_parts(d.cast(), size))
        }
    }
}

impl Drop for CairoImageSurface {
    fn drop(&mut self) {
        unsafe {
            cairo_surface_destroy(self.s);
        }
    }
}

pub struct CairoContext {
    _s: Rc<CairoImageSurface>,
    c: *mut cairo_t,
}

impl CairoContext {
    pub fn create_pango_context(self: &Rc<Self>) -> Result<Rc<PangoCairoContext>, PangoError> {
        unsafe {
            let p = pango_cairo_create_context(self.c);
            if p.is_null() {
                return Err(PangoError::CreatePangoCairo);
            }
            Ok(Rc::new(PangoCairoContext { c: self.clone(), p }))
        }
    }

    pub fn set_operator(&self, op: CairoOperator) {
        unsafe {
            cairo_set_operator(self.c, op.raw() as _);
        }
    }

    pub fn set_source_rgba(&self, r: f64, g: f64, b: f64, a: f64) {
        unsafe {
            cairo_set_source_rgba(self.c, r, g, b, a);
        }
    }

    pub fn move_to(&self, x: f64, y: f64) {
        unsafe {
            cairo_move_to(self.c, x, y);
        }
    }
}

impl Drop for CairoContext {
    fn drop(&mut self) {
        unsafe {
            cairo_destroy(self.c);
        }
    }
}

pub struct PangoCairoContext {
    c: Rc<CairoContext>,
    p: *mut PangoContext_,
}

impl PangoCairoContext {
    pub fn create_layout(self: &Rc<Self>) -> Result<PangoLayout, PangoError> {
        unsafe {
            let l = pango_layout_new(self.p as _);
            if l.is_null() {
                return Err(PangoError::CreateLayout);
            }
            Ok(PangoLayout { c: self.clone(), l })
        }
    }
}

impl Drop for PangoCairoContext {
    fn drop(&mut self) {
        unsafe {
            g_object_unref(self.p as _);
        }
    }
}

pub struct PangoFontDescription {
    s: *mut PangoFontDescription_,
}

impl PangoFontDescription {
    pub fn from_string<'a>(s: impl IntoUstr<'a>) -> Self {
        let s = s.into_ustr();
        Self {
            s: unsafe { pango_font_description_from_string(s.as_ptr()) },
        }
    }

    pub fn size(&self) -> i32 {
        unsafe { pango_font_description_get_size(self.s) as _ }
    }

    pub fn set_size(&mut self, size: i32) {
        unsafe {
            pango_font_description_set_size(self.s, size);
        }
    }
}

impl Drop for PangoFontDescription {
    fn drop(&mut self) {
        unsafe {
            pango_font_description_free(self.s);
        }
    }
}

pub struct PangoLayout {
    c: Rc<PangoCairoContext>,
    l: *mut PangoLayout_,
}

impl PangoLayout {
    pub fn set_width(&self, width: i32) {
        unsafe {
            pango_layout_set_width(self.l, width as _);
        }
    }

    pub fn set_ellipsize(&self, ellipsize: PangoEllipsizeMode) {
        unsafe {
            pango_layout_set_ellipsize(self.l, ellipsize.raw() as _);
        }
    }

    pub fn set_font_description(&self, fd: &PangoFontDescription) {
        unsafe {
            pango_layout_set_font_description(self.l, fd.s);
        }
    }

    pub fn set_text(&self, text: &str) {
        unsafe {
            pango_layout_set_text(self.l, text.as_ptr() as _, text.len() as _);
        }
    }

    pub fn set_markup(&self, text: &str) {
        unsafe {
            pango_layout_set_markup(self.l, text.as_ptr() as _, text.len() as _);
        }
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        unsafe {
            let mut w = 0;
            let mut h = 0;
            pango_layout_get_pixel_size(self.l, &mut w, &mut h);
            (w as _, h as _)
        }
    }

    pub fn inc_pixel_rect(&self) -> Rect {
        unsafe {
            let mut rect = PangoRectangle {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            };
            pango_layout_get_extents(self.l, &mut rect, ptr::null_mut());
            pango_extents_to_pixels(&mut rect, ptr::null_mut());
            Rect::new_sized(rect.x, rect.y, rect.width, rect.height).unwrap()
        }
    }

    pub fn show_layout(&self) {
        unsafe {
            pango_cairo_show_layout(self.c.c.c, self.l);
        }
    }
}

impl Drop for PangoLayout {
    fn drop(&mut self) {
        unsafe {
            g_object_unref(self.l as _);
        }
    }
}
