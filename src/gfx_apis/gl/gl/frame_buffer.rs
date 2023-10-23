use {
    crate::gfx_apis::gl::{
        egl::context::EglContext,
        gl::{
            render_buffer::GlRenderBuffer,
            sys::{glDeleteFramebuffers, GLuint},
            texture::GlTexture,
        },
    },
    std::rc::Rc,
};

pub struct GlFrameBuffer {
    pub _rb: Option<Rc<GlRenderBuffer>>,
    pub _tex: Option<Rc<GlTexture>>,
    pub ctx: Rc<EglContext>,
    pub width: i32,
    pub height: i32,
    pub fbo: GLuint,
}

impl Drop for GlFrameBuffer {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| {
            unsafe {
                glDeleteFramebuffers(1, &self.fbo);
            }
            Ok(())
        });
    }
}
