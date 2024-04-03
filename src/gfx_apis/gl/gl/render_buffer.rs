use {
    crate::{
        format::Format,
        gfx_apis::gl::{
            egl::{context::EglContext, image::EglImage},
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{
                    GLeglImageOES, GLuint, GL_COLOR_ATTACHMENT0, GL_FRAMEBUFFER,
                    GL_FRAMEBUFFER_COMPLETE, GL_RENDERBUFFER,
                },
            },
            RenderError,
        },
    },
    std::rc::Rc,
};

pub struct GlRenderBuffer {
    pub _img: Option<Rc<EglImage>>,
    pub ctx: Rc<EglContext>,
    pub width: i32,
    pub height: i32,
    pub format: &'static Format,
    rbo: GLuint,
}

impl GlRenderBuffer {
    pub(in crate::gfx_apis::gl) unsafe fn new(
        ctx: &Rc<EglContext>,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<Rc<GlRenderBuffer>, RenderError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(RenderError::UnsupportedShmFormat(format.name));
        };
        let gles = &ctx.dpy.gles;
        let mut rbo = 0;
        (gles.glGenRenderbuffers)(1, &mut rbo);
        (gles.glBindRenderbuffer)(GL_RENDERBUFFER, rbo);
        (gles.glRenderbufferStorage)(GL_RENDERBUFFER, shm_info.gl_internal_format, width, height);
        (gles.glBindRenderbuffer)(GL_RENDERBUFFER, 0);
        Ok(Rc::new(GlRenderBuffer {
            _img: None,
            ctx: ctx.clone(),
            width,
            height,
            format,
            rbo,
        }))
    }

    pub(in crate::gfx_apis::gl) unsafe fn from_image(
        img: &Rc<EglImage>,
        ctx: &Rc<EglContext>,
    ) -> Result<Rc<GlRenderBuffer>, RenderError> {
        if img.external_only {
            return Err(RenderError::ExternalOnly);
        }
        let gles = ctx.dpy.gles;
        let mut rbo = 0;
        (gles.glGenRenderbuffers)(1, &mut rbo);
        (gles.glBindRenderbuffer)(GL_RENDERBUFFER, rbo);
        ctx.dpy
            .procs
            .glEGLImageTargetRenderbufferStorageOES(GL_RENDERBUFFER, GLeglImageOES(img.img.0));
        (gles.glBindRenderbuffer)(GL_RENDERBUFFER, 0);
        Ok(Rc::new(GlRenderBuffer {
            _img: Some(img.clone()),
            ctx: ctx.clone(),
            width: img.dmabuf.width,
            height: img.dmabuf.height,
            format: img.dmabuf.format,
            rbo,
        }))
    }

    pub(in crate::gfx_apis::gl) unsafe fn create_framebuffer(
        self: &Rc<Self>,
    ) -> Result<GlFrameBuffer, RenderError> {
        let gles = self.ctx.dpy.gles;
        let mut fbo = 0;
        (gles.glGenFramebuffers)(1, &mut fbo);
        (gles.glBindFramebuffer)(GL_FRAMEBUFFER, fbo);
        (gles.glFramebufferRenderbuffer)(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_RENDERBUFFER,
            self.rbo,
        );
        let status = (gles.glCheckFramebufferStatus)(GL_FRAMEBUFFER);
        (gles.glBindFramebuffer)(GL_FRAMEBUFFER, 0);
        let fb = GlFrameBuffer {
            rb: self.clone(),
            _tex: None,
            ctx: self.ctx.clone(),
            fbo,
            width: self.width,
            height: self.height,
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
                (self.ctx.dpy.gles.glDeleteRenderbuffers)(1, &self.rbo);
            }
            Ok(())
        });
    }
}
