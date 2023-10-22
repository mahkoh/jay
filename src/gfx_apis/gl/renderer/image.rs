use {
    crate::gfx_apis::gl::{
        egl::image::EglImage,
        gl::{render_buffer::GlRenderBuffer, texture::GlTexture},
        Framebuffer, RenderContext, RenderError, Texture,
    },
    std::rc::Rc,
};

pub struct Image {
    pub(crate) ctx: Rc<RenderContext>,
    pub(crate) gl: Rc<EglImage>,
}

impl Image {
    pub fn width(&self) -> i32 {
        self.gl.width
    }

    pub fn height(&self) -> i32 {
        self.gl.height
    }

    pub fn to_texture(self: &Rc<Self>) -> Result<Rc<Texture>, RenderError> {
        Ok(Rc::new(Texture {
            ctx: self.ctx.clone(),
            gl: GlTexture::import_img(&self.ctx.ctx, &self.gl)?,
        }))
    }

    pub fn to_framebuffer(&self) -> Result<Rc<Framebuffer>, RenderError> {
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
