use {
    crate::gfx_apis::gl::{
        egl::{context::EglContext, image::EglImage, PROCS},
        gl::{
            frame_buffer::GlFrameBuffer,
            sys::{
                glBindFramebuffer, glBindRenderbuffer, glCheckFramebufferStatus,
                glDeleteRenderbuffers, glFramebufferRenderbuffer, glGenFramebuffers,
                glGenRenderbuffers, GLeglImageOES, GLuint, GL_COLOR_ATTACHMENT0, GL_FRAMEBUFFER,
                GL_FRAMEBUFFER_COMPLETE, GL_RENDERBUFFER,
            },
        },
        RenderError,
    },
    std::rc::Rc,
};

pub struct GlRenderBuffer {
    pub img: Rc<EglImage>,
    pub ctx: Rc<EglContext>,
    rbo: GLuint,
}

impl GlRenderBuffer {
    pub(in crate::gfx_apis::gl) unsafe fn from_image(
        img: &Rc<EglImage>,
        ctx: &Rc<EglContext>,
    ) -> Result<Rc<GlRenderBuffer>, RenderError> {
        if img.external_only {
            return Err(RenderError::ExternalOnly);
        }
        let mut rbo = 0;
        glGenRenderbuffers(1, &mut rbo);
        glBindRenderbuffer(GL_RENDERBUFFER, rbo);
        PROCS.glEGLImageTargetRenderbufferStorageOES(GL_RENDERBUFFER, GLeglImageOES(img.img.0));
        glBindRenderbuffer(GL_RENDERBUFFER, 0);
        Ok(Rc::new(GlRenderBuffer {
            img: img.clone(),
            ctx: ctx.clone(),
            rbo,
        }))
    }

    pub(in crate::gfx_apis::gl) unsafe fn create_framebuffer(
        self: &Rc<Self>,
    ) -> Result<GlFrameBuffer, RenderError> {
        let mut fbo = 0;
        glGenFramebuffers(1, &mut fbo);
        glBindFramebuffer(GL_FRAMEBUFFER, fbo);
        glFramebufferRenderbuffer(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_RENDERBUFFER,
            self.rbo,
        );
        let status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        let fb = GlFrameBuffer {
            rb: self.clone(),
            _tex: None,
            ctx: self.ctx.clone(),
            fbo,
            width: self.img.width,
            height: self.img.height,
        };
        if status != GL_FRAMEBUFFER_COMPLETE {
            return Err(RenderError::CreateFramebuffer);
        }
        Ok(fb)
    }
}

impl Drop for GlRenderBuffer {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| {
            unsafe {
                glDeleteRenderbuffers(1, &self.rbo);
            }
            Ok(())
        });
    }
}
