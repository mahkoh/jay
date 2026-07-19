use crate::allocator::BufferObject;
use crate::gfx_apis::gl::Framebuffer;
use crate::gfx_apis::gl::GlRenderContext;
use crate::gfx_apis::gl::RenderError;
use crate::gfx_apis::gl::Texture;
use crate::gfx_apis::gl::egl::image::EglImage;
use crate::gfx_apis::gl::gl::texture::GlTexture;
use std::rc::Rc;

pub struct Image {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: Rc<EglImage>,
    pub(in crate::gfx_apis::gl) _bo: Option<Rc<dyn BufferObject>>,
}

impl Image {
    pub fn to_texture(&self) -> Result<Rc<Texture>, RenderError> {
        Ok(Rc::new(Texture {
            ctx: self.ctx.clone(),
            gl: GlTexture::import_img(&self.ctx.ctx, &self.gl)?,
            format: self.gl.dmabuf.format,
        }))
    }

    pub fn to_framebuffer(&self) -> Result<Rc<Framebuffer>, RenderError> {
        self.ctx.image_to_fb(&self.gl)
    }
}
