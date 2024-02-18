use {
    crate::{
        gfx_api::{GfxError, GfxFramebuffer, GfxImage, GfxTexture},
        gfx_apis::gl::{
            egl::image::EglImage,
            gl::{render_buffer::GlRenderBuffer, texture::GlTexture},
            Framebuffer, GlRenderContext, RenderError, Texture,
        },
    },
    std::rc::Rc,
};

pub struct Image {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: Rc<EglImage>,
}

impl Image {
    pub fn width(&self) -> i32 {
        self.gl.dmabuf.width
    }

    pub fn height(&self) -> i32 {
        self.gl.dmabuf.height
    }

    fn to_texture(self: &Rc<Self>) -> Result<Rc<Texture>, RenderError> {
        Ok(Rc::new(Texture {
            ctx: self.ctx.clone(),
            gl: GlTexture::import_img(&self.ctx.ctx, &self.gl)?,
        }))
    }

    fn to_framebuffer(&self) -> Result<Rc<Framebuffer>, RenderError> {
        self.ctx.ctx.with_current(|| unsafe {
            let rb = GlRenderBuffer::from_image(&self.gl, &self.ctx.ctx)?;
            let fb = rb.create_framebuffer()?;
            Ok(Rc::new(Framebuffer {
                ctx: self.ctx.clone(),
                gl: fb,
            }))
        })
    }
}

impl GfxImage for Image {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        (*self)
            .to_framebuffer()
            .map(|v| v as Rc<dyn GfxFramebuffer>)
            .map_err(|e| e.into())
    }

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError> {
        (&self)
            .to_texture()
            .map(|v| v as Rc<dyn GfxTexture>)
            .map_err(|e| e.into())
    }

    fn width(&self) -> i32 {
        self.width()
    }

    fn height(&self) -> i32 {
        self.height()
    }
}
