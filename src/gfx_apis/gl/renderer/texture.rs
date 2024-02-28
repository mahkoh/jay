use {
    crate::{
        format::Format,
        gfx_api::{GfxError, GfxTexture, TextureReservations},
        gfx_apis::gl::{
            gl::texture::GlTexture,
            renderer::{context::GlRenderContext, framebuffer::Framebuffer},
            RenderError,
        },
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
    pub(in crate::gfx_apis::gl) resv: TextureReservations,
    pub(in crate::gfx_apis::gl) format: &'static Format,
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

    pub fn to_framebuffer(&self) -> Result<Rc<Framebuffer>, RenderError> {
        match &self.gl.img {
            Some(img) => self.ctx.image_to_fb(img),
            _ => Err(RenderError::ShmTextureToFb),
        }
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
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        _stride: i32,
        format: &Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        self.to_framebuffer()?
            .copy_to_shm(x, y, width, height, format, shm);
        Ok(())
    }

    fn dmabuf(&self) -> Option<&DmaBuf> {
        self.gl.img.as_ref().map(|i| &i.dmabuf)
    }

    fn reservations(&self) -> &TextureReservations {
        &self.resv
    }

    fn format(&self) -> &'static Format {
        self.format
    }
}
