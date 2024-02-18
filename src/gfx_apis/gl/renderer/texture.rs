use {
    crate::{
        format::Format,
        gfx_api::{GfxError, GfxTexture},
        gfx_apis::gl::{gl::texture::GlTexture, renderer::context::GlRenderContext, RenderError},
        video::dmabuf::DmaBuf,
    },
    std::{
        any::Any,
        cell::Cell,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Texture {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: GlTexture,
}

impl Debug for Texture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture").finish_non_exhaustive()
    }
}

impl Texture {
    pub fn width(&self) -> i32 {
        self.gl.width
    }

    pub fn height(&self) -> i32 {
        self.gl.height
    }
}

impl GfxTexture for Texture {
    fn size(&self) -> (i32, i32) {
        (self.width(), self.height())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn read_pixels(
        self: Rc<Self>,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
        _stride: i32,
        _format: &Format,
        _shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        Err(RenderError::UnsupportedOperation.into())
    }

    fn dmabuf(&self) -> Option<&DmaBuf> {
        self.gl.img.as_ref().map(|i| &i.dmabuf)
    }
}
